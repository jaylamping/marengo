//! Gravity calibration on the live URDF: synthetic torques from a perturbed copy of the
//! model (independent `gravity_torques` path), Coulomb friction with approach-dependent sign,
//! measurement noise, and the live soft ∩ hard joint windows.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use armee_dynamics::calibration::{
    assess_identifiability, cancel_friction, default_params, fit_gravity_params, CalibrationError,
    FitOptions, FitVerdict, GravitySample, InertialParam, JointWindow,
};
use armee_dynamics::{gravity_model_from_urdf, DynamicsModel, LinkInertial, UrdfGravityModel};

const PITCH: &str = "right_shoulder_pitch";
const ELBOW: &str = "right_elbow_pitch";
const UPPER_ARM: &str = "right_upper_arm_link";
const FOREARM: &str = "right_forearm_link";
/// Static friction magnitude per joint (Nm); sign follows the approach direction.
const FRICTION_NM: f64 = 0.08;
/// Uniform noise half-width on each averaged torque (Nm).
const NOISE_NM: f64 = 0.01;
/// Settled position short of target, toward the approach side (rad).
const SETTLE_SHORT_RAD: f64 = 0.004;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn live() -> (UrdfGravityModel, Vec<JointWindow>) {
    let config = repo_root().join("config");
    let robot = marengo_config::load_robot_config_from(&config).expect("robot.yaml");
    let control = marengo_config::load_control_config_from(&config).expect("control.yaml");
    let motors = marengo_config::load_motors_config_from(&config).expect("motors.yaml");
    let model = gravity_model_from_urdf(repo_root().join(&robot.robot.urdf), &robot.robot.joints)
        .expect("live URDF");
    let windows = robot
        .robot
        .joints
        .iter()
        .map(|j| {
            let (lower_rad, upper_rad) =
                marengo_config::commanded_position_window(&control, &motors, j)
                    .expect("soft ∩ hard window");
            JointWindow {
                joint: j.clone(),
                lower_rad,
                upper_rad,
            }
        })
        .collect();
    (model, windows)
}

fn index(model: &UrdfGravityModel, joint: &str) -> usize {
    model
        .joint_names()
        .iter()
        .position(|j| j == joint)
        .expect("joint in model")
}

/// Pose with `sweep` at each angle and the remaining joints at `base`.
fn sweep(
    model: &UrdfGravityModel,
    base: &[(&str, f64)],
    sweep: &str,
    angles: &[f64],
) -> Vec<Vec<f64>> {
    angles
        .iter()
        .map(|&a| {
            let mut q = vec![0.0; model.joint_names().len()];
            for &(j, v) in base {
                q[index(model, j)] = v;
            }
            q[index(model, sweep)] = a;
            q
        })
        .collect()
}

fn scaled(
    model: &UrdfGravityModel,
    link: &str,
    mass_scale: f64,
    com_offset_m: f64,
) -> UrdfGravityModel {
    let cad = model.link_inertial(link).expect("link");
    let norm = cad.com_m.iter().map(|v| v * v).sum::<f64>().sqrt();
    let com_m = cad.com_m.map(|v| v + com_offset_m * v / norm);
    model
        .with_link_inertial(
            link,
            LinkInertial {
                mass_kg: cad.mass_kg * mass_scale,
                com_m,
            },
        )
        .expect("perturbed model")
}

/// Deterministic uniform noise in [-1, 1].
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }
}

/// Bench measurement of `truth` at each pose: approach from below and from above along the
/// swept joint (friction ± and settling short of target), noise, then friction cancellation.
fn measure(
    truth: &UrdfGravityModel,
    poses: &[Vec<f64>],
    swept: usize,
    bias: &[(usize, f64)],
    noise: &mut Noise,
) -> Vec<GravitySample> {
    poses
        .iter()
        .enumerate()
        .map(|(k, pose)| {
            let approach = |dir: f64, noise: &mut Noise| {
                let mut q = pose.clone();
                q[swept] -= dir * SETTLE_SHORT_RAD;
                let tau = truth
                    .gravity_torques(&q)
                    .expect("truth torques")
                    .iter()
                    .enumerate()
                    .map(|(j, t)| {
                        let b = bias.iter().find(|(i, _)| *i == j).map_or(0.0, |(_, b)| *b);
                        t + dir * FRICTION_NM + b + NOISE_NM * noise.next()
                    })
                    .collect::<Vec<_>>();
                (q, tau)
            };
            let (q_below, tau_below) = approach(1.0, noise);
            let (q_above, tau_above) = approach(-1.0, noise);
            let (tau_meas, friction) = cancel_friction(&tau_below, &tau_above);
            for f in friction {
                assert!(
                    (f.abs() - FRICTION_NM).abs() < 0.05,
                    "friction estimate {f}"
                );
            }
            GravitySample {
                label: format!("pose{k}"),
                q: q_below
                    .iter()
                    .zip(&q_above)
                    .map(|(b, a)| (b + a) / 2.0)
                    .collect(),
                tau_meas,
            }
        })
        .collect()
}

fn fitted(fit: &armee_dynamics::calibration::GravityFit, param: &str) -> f64 {
    fit.params
        .iter()
        .find(|p| p.param.to_string() == param)
        .unwrap_or_else(|| panic!("{param} not fitted"))
        .fitted
}

const PITCH_POSES: [f64; 5] = [0.0, 0.25, 0.48, 0.8, 1.2];

#[test]
fn pitch_sweep_recovers_upper_arm_and_forearm_mass() {
    let (model, windows) = live();
    let truth = scaled(&scaled(&model, UPPER_ARM, 1.2, 0.0), FOREARM, 1.25, 0.0);
    let poses = sweep(&model, &[], PITCH, &PITCH_POSES);
    let opts = FitOptions::default();
    let params = default_params(&model, PITCH, &poses, &[], &opts).expect("default params");
    let mut names: Vec<String> = params.iter().map(ToString::to_string).collect();
    names.sort();
    assert_eq!(
        names,
        [format!("mass:{FOREARM}"), format!("mass:{UPPER_ARM}")]
    );

    let samples = measure(
        &truth,
        &poses,
        index(&model, PITCH),
        &[],
        &mut Noise(0x9E37_79B9),
    );
    let fit = fit_gravity_params(&model, &params, &samples, &windows, &[], &opts).expect("fit");
    assert_eq!(
        fit.verdict,
        FitVerdict::Accepted,
        "{:#?}",
        fit.identifiability
    );
    assert!(
        (fitted(&fit, &format!("mass:{UPPER_ARM}")) - 1.2).abs() < 0.04,
        "{:?}",
        fit.params
    );
    assert!(
        (fitted(&fit, &format!("mass:{FOREARM}")) - 1.25).abs() < 0.04,
        "{:?}",
        fit.params
    );
    assert!(
        fit.max_abs_before_nm > 0.10,
        "perturbation must be visible: {}",
        fit.max_abs_before_nm
    );
    assert!(
        fit.max_abs_after_nm < 0.05,
        "residual after fit {}",
        fit.max_abs_after_nm
    );
    let forearm = fit
        .links
        .iter()
        .find(|l| l.link == FOREARM)
        .expect("forearm");
    assert_eq!(
        forearm.fitted.com_m, forearm.cad.com_m,
        "mass-only fit keeps COM"
    );
}

#[test]
fn pitch_and_elbow_sweeps_recover_forearm_com_offset() {
    let (model, windows) = live();
    let truth = scaled(&model, FOREARM, 1.1, 0.015);
    let mut poses = sweep(&model, &[], PITCH, &PITCH_POSES);
    let elbow_poses = sweep(&model, &[(PITCH, 0.48)], ELBOW, &[0.0, 0.25, 0.5, 0.75]);
    let mut noise = Noise(0xDEAD_BEEF);
    let mut samples = measure(&truth, &poses, index(&model, PITCH), &[], &mut noise);
    samples.extend(measure(
        &truth,
        &elbow_poses,
        index(&model, ELBOW),
        &[],
        &mut noise,
    ));
    poses.extend(elbow_poses);

    let params = [InertialParam::mass(FOREARM), InertialParam::com(FOREARM)];
    let opts = FitOptions::default();
    let fit = fit_gravity_params(&model, &params, &samples, &windows, &[], &opts).expect("fit");
    assert_eq!(
        fit.verdict,
        FitVerdict::Accepted,
        "{:#?}",
        fit.identifiability
    );
    assert!(
        (fitted(&fit, &format!("mass:{FOREARM}")) - 1.1).abs() < 0.04,
        "{:?}",
        fit.params
    );
    assert!(
        (fitted(&fit, &format!("com:{FOREARM}")) - 0.015).abs() < 0.003,
        "{:?}",
        fit.params
    );
    let forearm = fit
        .links
        .iter()
        .find(|l| l.link == FOREARM)
        .expect("forearm");
    let moved = forearm
        .fitted
        .com_m
        .iter()
        .zip(&forearm.cad.com_m)
        .map(|(f, c)| (f - c).powi(2))
        .sum::<f64>()
        .sqrt();
    assert!(
        (moved - 0.015).abs() < 0.003,
        "COM moved {moved} m along the axis"
    );
}

#[test]
fn refuses_parameters_the_sweep_cannot_separate() {
    let (model, windows) = live();
    let poses = sweep(&model, &[], PITCH, &PITCH_POSES);
    let params = [
        InertialParam::mass(UPPER_ARM),
        InertialParam::com(UPPER_ARM),
    ];
    let opts = FitOptions::default();
    let ident = assess_identifiability(&model, &params, &poses, &[], &opts).expect("ident");
    assert!(
        ident.min_singular_nm < opts.min_identifiable_nm,
        "{ident:?}"
    );

    let samples = measure(&model, &poses, index(&model, PITCH), &[], &mut Noise(7));
    let fit = fit_gravity_params(&model, &params, &samples, &windows, &[], &opts).expect("fit");
    assert!(
        matches!(fit.verdict, FitVerdict::IllConditioned { .. }),
        "{:?}",
        fit.verdict
    );
    assert!(!fit.accepted());
}

#[test]
fn refuses_when_the_fit_cannot_explain_the_torques() {
    let (model, windows) = live();
    let poses = sweep(&model, &[], PITCH, &PITCH_POSES);
    // A cable pulling the lower-arm yaw is not a link inertial: no mass change explains it.
    let lower_yaw = index(&model, "right_lower_arm_yaw");
    let samples = measure(
        &model,
        &poses,
        index(&model, PITCH),
        &[(lower_yaw, 0.2)],
        &mut Noise(11),
    );
    let opts = FitOptions::default();
    let params = default_params(&model, PITCH, &poses, &[], &opts).expect("params");
    let fit = fit_gravity_params(&model, &params, &samples, &windows, &[], &opts).expect("fit");
    match fit.verdict {
        FitVerdict::ResidualTooHigh {
            max_abs_nm,
            limit_nm,
        } => {
            assert!(max_abs_nm > limit_nm);
        }
        other => panic!("expected ResidualTooHigh, got {other:?}"),
    }
}

#[test]
fn refuses_poses_outside_the_soft_window() {
    let (model, windows) = live();
    let pitch = windows
        .iter()
        .find(|w| w.joint == PITCH)
        .expect("pitch window");
    let opts = FitOptions::default();
    let beyond = pitch.upper_rad + opts.pose_limit_slack_rad + 0.01;
    let poses = sweep(&model, &[], PITCH, &[0.0, 0.48, beyond]);
    let samples = measure(&model, &poses, index(&model, PITCH), &[], &mut Noise(3));
    let err = fit_gravity_params(
        &model,
        &[InertialParam::mass(FOREARM)],
        &samples,
        &windows,
        &[],
        &opts,
    )
    .expect_err("out-of-limit pose");
    assert!(
        matches!(&err, CalibrationError::PoseOutOfLimits { joint, .. } if joint == PITCH),
        "{err}"
    );

    let missing: Vec<JointWindow> = windows
        .iter()
        .filter(|w| w.joint != ELBOW)
        .cloned()
        .collect();
    let ok_poses = sweep(&model, &[], PITCH, &PITCH_POSES);
    let ok = measure(&model, &ok_poses, index(&model, PITCH), &[], &mut Noise(5));
    let err = fit_gravity_params(
        &model,
        &[InertialParam::mass(FOREARM)],
        &ok,
        &missing,
        &[],
        &opts,
    )
    .expect_err("missing window");
    assert!(
        matches!(err, CalibrationError::MissingWindow { .. }),
        "{err}"
    );
}

#[test]
fn rejects_bad_parameter_specs() {
    assert!("mass:right_forearm_link".parse::<InertialParam>().is_ok());
    assert!("com:right_forearm_link".parse::<InertialParam>().is_ok());
    for bad in [
        "inertia:right_forearm_link",
        "mass:",
        "right_forearm_link",
        "mass:a b",
    ] {
        assert!(bad.parse::<InertialParam>().is_err(), "{bad}");
    }
    let (model, windows) = live();
    let poses = sweep(&model, &[], PITCH, &PITCH_POSES);
    let samples = measure(&model, &poses, index(&model, PITCH), &[], &mut Noise(9));
    let opts = FitOptions::default();
    let dup = [InertialParam::mass(FOREARM), InertialParam::mass(FOREARM)];
    assert!(matches!(
        fit_gravity_params(&model, &dup, &samples, &windows, &[], &opts),
        Err(CalibrationError::DuplicateParam { .. })
    ));
    assert!(matches!(
        fit_gravity_params(&model, &[], &samples, &windows, &[], &opts),
        Err(CalibrationError::NoParams)
    ));
}

#[test]
fn point_mass_torques_reproduce_link_contribution() {
    let (model, _) = live();
    let cad = model.link_inertial(FOREARM).expect("forearm");
    let heavier = scaled(&model, FOREARM, 2.0, 0.0);
    let q = sweep(&model, &[(PITCH, 0.7)], ELBOW, &[0.4]).remove(0);
    let delta: Vec<f64> = heavier
        .gravity_torques(&q)
        .expect("heavier")
        .iter()
        .zip(model.gravity_torques(&q).expect("cad").iter())
        .map(|(h, c)| h - c)
        .collect();
    let unit = model
        .point_mass_torques(FOREARM, cad.com_m, &q)
        .expect("point");
    for (d, u) in delta.iter().zip(&unit) {
        assert!(
            (d - cad.mass_kg * u).abs() < 1e-6,
            "{d} vs {}",
            cad.mass_kg * u
        );
    }
}

/// L9/L10: when the normal equations are singular the loop used to `break` silently, the
/// verdict looked at an unconverged θ, and the posterior σ came out as `NaN.max(0.0) = 0`
/// ("certain"). A parameter on a link no joint moves has an all-zero Jacobian column; with a
/// flat prior (λ = 0) the normal matrix is exactly singular.
#[test]
fn singular_normal_equations_refuse_and_never_report_zero_sigma() {
    let (model, windows) = live();
    let poses = sweep(&model, &[], PITCH, &PITCH_POSES);
    let samples = measure(&model, &poses, index(&model, PITCH), &[], &mut Noise(3));
    let params = [
        InertialParam::mass("base_link"),
        InertialParam::mass(UPPER_ARM),
    ];
    let opts = FitOptions {
        prior_mass_scale_sigma: f64::INFINITY,
        ..FitOptions::default()
    };
    let fit = fit_gravity_params(&model, &params, &samples, &windows, &[], &opts)
        .expect("a singular fit is a refusal verdict, not an input error");
    assert!(!fit.accepted());
    assert!(
        matches!(fit.verdict, FitVerdict::NotConverged { .. }),
        "{:?}",
        fit.verdict
    );
    for p in &fit.params {
        assert!(
            p.sigma.is_nan(),
            "σ of {} must be NaN (unknown), not {}",
            p.param,
            p.sigma
        );
    }
}

#[test]
fn non_finite_weights_are_not_reported_as_a_fit() {
    let (model, windows) = live();
    let poses = sweep(&model, &[], PITCH, &PITCH_POSES);
    let samples = measure(&model, &poses, index(&model, PITCH), &[], &mut Noise(5));
    let params = [InertialParam::mass(UPPER_ARM), InertialParam::mass(FOREARM)];
    for torque_sigma_nm in [f64::NAN, 0.0] {
        let opts = FitOptions {
            torque_sigma_nm,
            ..FitOptions::default()
        };
        let fit = fit_gravity_params(&model, &params, &samples, &windows, &[], &opts)
            .expect("refusal verdict");
        assert!(!fit.accepted(), "σ_τ={torque_sigma_nm}: {:?}", fit.verdict);
        assert!(
            fit.params.iter().all(|p| p.sigma != 0.0),
            "σ_τ={torque_sigma_nm}: {:?}",
            fit.params
        );
    }
}

#[test]
fn accepted_fit_reports_a_finite_positive_sigma() {
    let (model, windows) = live();
    let truth = scaled(&model, UPPER_ARM, 1.2, 0.0);
    let poses = sweep(&model, &[], PITCH, &PITCH_POSES);
    let samples = measure(&truth, &poses, index(&model, PITCH), &[], &mut Noise(9));
    let opts = FitOptions::default();
    let params = default_params(&model, PITCH, &poses, &[], &opts).expect("params");
    let fit = fit_gravity_params(&model, &params, &samples, &windows, &[], &opts).expect("fit");
    assert!(fit.accepted(), "{:?}", fit.verdict);
    assert!(fit
        .params
        .iter()
        .all(|p| p.sigma.is_finite() && p.sigma > 0.0));
}
