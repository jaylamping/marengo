//! `marengo-log-cli gravity-fit` on synthetic **wave-method** sessions (`pi_joint_calibrate`
//! `method: "wave"`): at each pose a local raised-cosine wave at three speeds, simulated on a
//! 1-DoF pitch plant with Stribeck friction and stiction (the arm sticks at every turnaround
//! until the P torque breaks it free), a CAD feed-forward that is wrong, and a slow torque
//! disturbance that differs between sessions. The fit must recover the true lumped
//! `A·sin q + B·cos q` of the pitch and its Coulomb/viscous friction.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::f64::consts::{FRAC_PI_2, PI};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use armee_dynamics::{gravity_model_from_urdf, DynamicsModel, LinkInertial, UrdfGravityModel};
use assert_cmd::prelude::*;
use serde_json::{json, Value};
use tempfile::TempDir;

const PITCH: &str = "right_shoulder_pitch";
const UPPER_ARM: &str = "right_upper_arm_link";
const FOREARM: &str = "right_forearm_link";
const HEADER: &str = "tick,t_ms,joint,q,dq,q_traj,dq_traj,q_des,target,target_raw,q_env_lo,q_env_hi,lead,lead_sat,settle_error,phase,friction_mode,tau_p,tau_g,tau_f,tau_d,tau_ff_cmd,tau_meas,dq_mit,kp,kd,joint_stuck,planner_frozen,retarget_age_ms,planner_event,law,q_ref,dq_ref,time_scale,tau_i,kd_mit,tau_ff_wire";

const POSES: [f64; 5] = [-0.5, -0.25, 0.0, 0.25, 0.5];
const AMPLITUDE: f64 = 0.06;
const SPEEDS: [f64; 3] = [0.25, 0.4, 0.55];
const CYCLES: u32 = 2;
const KP: f64 = 18.0;
const KD: f64 = 3.0;
/// True friction: Coulomb, viscous, breakaway, Stribeck velocity.
const FC: f64 = 0.45;
const FV: f64 = 0.10;
const FS: f64 = 0.60;
const VB: f64 = 0.05;
const SIM_DT: f64 = 0.001;
const LOG_EVERY: usize = 5;
/// RS03 position feedback step (4π / 32767).
const Q_STEP: f64 = 4.0 * PI / 32767.0;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn joints() -> Vec<String> {
    marengo_config::load_robot_config_from(repo().join("config"))
        .expect("robot.yaml")
        .robot
        .joints
}

fn cad() -> UrdfGravityModel {
    gravity_model_from_urdf(repo().join("assets/urdf/marengo.urdf"), &joints()).expect("URDF")
}

/// The "real" arm: lighter forearm, upper-arm COM off to the side (moves B).
fn truth() -> UrdfGravityModel {
    let model = cad();
    let fa = model.link_inertial(FOREARM).unwrap();
    let ua = model.link_inertial(UPPER_ARM).unwrap();
    model
        .with_link_inertial(
            FOREARM,
            LinkInertial {
                mass_kg: fa.mass_kg * 0.8,
                com_m: fa.com_m,
            },
        )
        .unwrap()
        .with_link_inertial(
            UPPER_ARM,
            LinkInertial {
                mass_kg: ua.mass_kg,
                com_m: [ua.com_m[0] + 0.02, ua.com_m[1], ua.com_m[2]],
            },
        )
        .unwrap()
}

/// Pitch `(A, B)` of a model at the all-zero pose of the other joints.
fn lumped(model: &UrdfGravityModel) -> (f64, f64) {
    let n = joints().len();
    let mut q = vec![0.0; n];
    q[0] = FRAC_PI_2;
    let a = model.gravity_torques(&q).unwrap()[0];
    q[0] = 0.0;
    let b = model.gravity_torques(&q).unwrap()[0];
    (a, b)
}

struct Session {
    ts: &'static str,
    /// Phases of the slow torque disturbance (sensor drift), per session.
    phase: (f64, f64),
    /// Coulomb friction of this session (temperature drift between sessions).
    fc: f64,
    /// Peak wave speeds (rad/s).
    speeds: &'static [f64],
    /// Amplitude of a q-locked torque readout ripple (Nm), identical in every session: it
    /// scatters the samples of a bin without changing what a repeat of the bin reads.
    ripple_nm: f64,
    /// Pose-dependent Coulomb friction `fc + this·cos(4π q)` (Nm), identical in every session.
    friction_ripple_nm: f64,
    /// One extra single-speed wave at this pose with a constant readout bias (Nm): a pose
    /// only this session sees once, with a gravity artefact.
    extra: Option<(f64, f64)>,
}

/// Period of the readout ripple (rad), a few gear teeth across a bin.
const RIPPLE_PERIOD_RAD: f64 = 0.004;

fn half_period(speed: f64) -> f64 {
    (PI * AMPLITUDE / speed * 100.0).ceil() / 100.0
}

struct Plant<'a> {
    truth: &'a UrdfGravityModel,
    /// Pitch `(A, B)` of the truth and of the CAD feed-forward.
    truth_ab: (f64, f64),
    cad_ab: (f64, f64),
    inertia: f64,
    fc: f64,
    ripple_nm: f64,
    friction_ripple_nm: f64,
    /// Readout bias of the current step (Nm).
    bias_nm: f64,
    q: f64,
    v: f64,
    stuck: bool,
    t: f64,
    tick: u64,
    step: usize,
    phase: (f64, f64),
    csv: String,
    joints: Vec<String>,
}

impl Plant<'_> {
    fn pose(&self, pitch: f64) -> Vec<f64> {
        let mut q = vec![0.0; self.joints.len()];
        q[0] = pitch;
        q
    }

    /// Advance one simulation step toward reference `(r, r_dot)`; log every LOG_EVERY steps.
    fn advance(&mut self, r: f64, r_dot: f64) {
        let tau_cad = self.cad_ab.0 * self.q.sin() + self.cad_ab.1 * self.q.cos();
        let tau_true = self.truth_ab.0 * self.q.sin() + self.truth_ab.1 * self.q.cos();
        let motor = tau_cad + KP * (r - self.q) + KD * (r_dot - self.v);
        let net = motor - tau_true;
        // Stuck until the net torque beats breakaway friction FS.
        if self.stuck && net.abs() > FS {
            self.stuck = false;
            self.v = net.signum() * 1e-6;
        }
        if !self.stuck {
            let fc = self.fc + self.friction_ripple_nm * (4.0 * PI * self.q).cos();
            let friction =
                self.v.signum() * (fc + (FS - fc) * (-self.v.abs() / VB).exp()) + FV * self.v;
            let v_next = self.v + (net - friction) / self.inertia * SIM_DT;
            if v_next * self.v < 0.0 {
                // Friction cannot reverse motion: the joint stops and sticks.
                self.v = 0.0;
                self.stuck = true;
            } else {
                self.v = v_next;
            }
            self.q += self.v * SIM_DT;
        }
        if self.step % LOG_EVERY == 0 {
            let disturbance = 0.012 * (2.0 * PI * 0.37 * self.t + self.phase.0).sin()
                + 0.008 * (2.0 * PI * 1.3 * self.t + self.phase.1).sin()
                + self.ripple_nm * (2.0 * PI * self.q / RIPPLE_PERIOD_RAD).sin()
                + self.bias_nm;
            let q_meas = (self.q / Q_STEP).round() * Q_STEP;
            let tau_g_all = self.truth.gravity_torques(&self.pose(self.q)).unwrap();
            for (j, name) in self.joints.iter().enumerate() {
                let (q, dq, target, meas, ff) = if j == 0 {
                    (q_meas, self.v, r, motor + disturbance, tau_cad)
                } else {
                    (0.0, 0.0, 0.0, tau_g_all[j], tau_g_all[j])
                };
                let _ = writeln!(
                    self.csv,
                    "{},{},{name},{q:.6},{dq:.6},0,0,0,{target:.6},{target:.6},0,0,0,0,0,Wave,static,{:.6},{ff:.6},0,0,{ff:.6},{meas:.6},0,18,3,0,0,0,tick,legacy,0.000000,0.000000,1.000000,0.000000,3.000,{ff:.6}",
                    self.tick,
                    self.tick * 5,
                    meas - ff,
                );
            }
            self.tick += 1;
        }
        self.step += 1;
        self.t += SIM_DT;
    }

    fn hold(&mut self, target: f64, seconds: f64) {
        for _ in 0..(seconds / SIM_DT).round() as usize {
            self.advance(target, 0.0);
        }
    }

    fn wave(&mut self, min: f64, max: f64, half_period_s: f64) {
        let mid = (min + max) / 2.0;
        let amp = (max - min) / 2.0;
        let n = (f64::from(CYCLES) * 2.0 * half_period_s / SIM_DT).round() as usize;
        for k in 0..=n {
            let phase = PI * k as f64 * SIM_DT / half_period_s;
            let r = mid - amp * phase.cos();
            let r_dot = amp * PI / half_period_s * phase.sin();
            self.advance(r, r_dot);
        }
    }
}

fn write_session(s: &Session) -> TempDir {
    let dir = TempDir::new().expect("tmp");
    let d = dir.path();
    std::fs::create_dir_all(d.join("config")).expect("config dir");
    for f in ["robot.yaml", "control.yaml", "motors.yaml"] {
        std::fs::copy(repo().join("config").join(f), d.join("config").join(f)).expect("copy");
    }
    std::fs::copy(
        repo().join("assets/urdf/marengo.urdf"),
        d.join("pi-marengo.urdf"),
    )
    .expect("urdf");
    let mut poses = POSES.to_vec();
    let mut steps = Vec::new();
    for (i, &pose) in POSES.iter().enumerate() {
        steps.push(
            json!({"kind": "hold", "joint": PITCH, "target_rad": pose - AMPLITUDE,
            "measure": false}),
        );
        for &speed in s.speeds {
            steps.push(
                json!({"kind": "wave", "joint": PITCH, "min_rad": pose - AMPLITUDE,
                "max_rad": pose + AMPLITUDE, "cycles": CYCLES,
                "half_period_s": half_period(speed), "pose_index": i}),
            );
        }
    }
    if let Some((pose, _)) = s.extra {
        poses.push(pose);
        steps.push(
            json!({"kind": "hold", "joint": PITCH, "target_rad": pose - AMPLITUDE,
            "measure": false}),
        );
        steps.push(
            json!({"kind": "wave", "joint": PITCH, "min_rad": pose - AMPLITUDE,
            "max_rad": pose + AMPLITUDE, "cycles": CYCLES,
            "half_period_s": half_period(s.speeds[0]), "pose_index": POSES.len()}),
        );
    }
    let plan = json!({
        "version": 2,
        "method": "wave",
        "created_utc": "2026-10-04T12:00:00.000Z",
        "session_ts": s.ts,
        "profile": "arm_attached",
        "sweep_joint": PITCH,
        "fixed_rad": {},
        "poses_rad": poses,
        "wave_amplitude_rad": AMPLITUDE,
        "settle_sec": 1.0,
        "steps": steps,
        "gravity_gate_report": "synthetic",
        "session_complete": true,
    });
    std::fs::write(
        d.join("plan.json"),
        serde_json::to_string_pretty(&plan).unwrap(),
    )
    .unwrap();

    let truth = truth();
    let joints = joints();
    let inertia = truth
        .joint_inertia(PITCH, &vec![0.0; joints.len()])
        .unwrap();
    let mut plant = Plant {
        truth: &truth,
        truth_ab: lumped(&truth),
        cad_ab: lumped(&cad()),
        inertia,
        fc: s.fc,
        ripple_nm: s.ripple_nm,
        friction_ripple_nm: s.friction_ripple_nm,
        bias_nm: 0.0,
        q: 0.0,
        v: 0.0,
        stuck: true,
        t: 0.0,
        tick: 0,
        step: 0,
        phase: s.phase,
        csv: format!("{HEADER}\n"),
        joints,
    };
    plant.hold(0.0, 0.2);
    for &pose in &POSES {
        plant.hold(pose - AMPLITUDE, 1.0);
        for &speed in s.speeds {
            plant.wave(pose - AMPLITUDE, pose + AMPLITUDE, half_period(speed));
            plant.hold(pose - AMPLITUDE, 0.5);
        }
    }
    if let Some((pose, bias)) = s.extra {
        plant.hold(pose - AMPLITUDE, 1.0);
        plant.bias_nm = bias;
        plant.wave(pose - AMPLITUDE, pose + AMPLITUDE, half_period(s.speeds[0]));
        plant.bias_nm = 0.0;
        plant.hold(pose - AMPLITUDE, 0.5);
    }
    // Return to 0 after the last step, as the session script does.
    plant.hold(0.0, 2.0);
    std::fs::write(d.join("position-trace.csv"), &plant.csv).unwrap();
    dir
}

fn run(sessions: &[&Path], out: &Path) -> std::process::Output {
    let mut cmd = Command::cargo_bin("marengo-log-cli").unwrap();
    cmd.arg("gravity-fit");
    for s in sessions {
        cmd.arg("--dir").arg(s);
    }
    cmd.arg("--out-dir")
        .arg(out)
        .arg("--repo-urdf")
        .arg(repo().join("assets/urdf/marengo.urdf"));
    cmd.output().unwrap()
}

fn record(out: &Path) -> Value {
    let path = std::fs::read_dir(out)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "json"))
        .expect("record json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn file_ending(out: &Path, suffix: &str) -> Option<PathBuf> {
    std::fs::read_dir(out)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.to_string_lossy().ends_with(suffix))
}

const SESSION_A: Session = Session {
    ts: "20261004T130000Z",
    phase: (0.3, 1.9),
    fc: FC,
    speeds: &SPEEDS,
    ripple_nm: 0.0,
    friction_ripple_nm: 0.0,
    extra: None,
};
const SESSION_B: Session = Session {
    ts: "20261004T131000Z",
    phase: (2.4, 0.7),
    fc: FC + 0.02,
    speeds: &SPEEDS,
    ripple_nm: 0.0,
    friction_ripple_nm: 0.0,
    extra: None,
};

#[test]
fn wave_sessions_recover_lumped_gravity_and_friction() {
    let a = write_session(&SESSION_A);
    let b = write_session(&SESSION_B);
    let out = TempDir::new().unwrap();
    let o = run(&[a.path(), b.path()], out.path());
    let stdout = String::from_utf8_lossy(&o.stdout);
    let rec = record(out.path());
    assert_eq!(
        o.status.code(),
        Some(0),
        "{stdout}\n{}\n{rec:#}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(rec["method"], "wave", "{rec:#}");
    let group = &rec["groups"][0];
    assert_eq!(group["joint"], PITCH);
    assert_eq!(group["accepted"], true, "{group:#}");
    let (a_true, b_true) = lumped(&truth());
    let (a_cad, b_cad) = lumped(&cad());
    assert!((a_true - a_cad).abs() > 0.2 && (b_true - b_cad).abs() > 0.05);
    let a_fit = group["fit"]["a_nm"].as_f64().unwrap();
    let b_fit = group["fit"]["b_nm"].as_f64().unwrap();
    assert!(
        (a_fit - a_true).abs() < 0.01,
        "A {a_fit} vs {a_true}: {group:#}"
    );
    assert!(
        (b_fit - b_true).abs() < 0.01,
        "B {b_fit} vs {b_true}: {group:#}"
    );
    // The gate is derived from the measured cross-session spread and reported.
    let gate = &group["gate"];
    assert!(
        gate["cross_session_sigma_nm"].as_f64().unwrap() > 0.0,
        "{gate:#}"
    );
    assert!(
        gate["residual_gate_nm"].as_f64().unwrap() <= 0.10,
        "{gate:#}"
    );

    // The proposed URDF reproduces the fitted A, B; masses stay as captured.
    let proposed = file_ending(out.path(), ".proposed-marengo.urdf").expect("proposed URDF");
    let patched = gravity_model_from_urdf(&proposed, &joints()).unwrap();
    let (a_p, b_p) = lumped(&patched);
    assert!((a_p - a_fit).abs() < 0.005 && (b_p - b_fit).abs() < 0.005);
    let patch = std::fs::read_to_string(file_ending(out.path(), ".urdf.patch").unwrap()).unwrap();
    assert!(
        patch
            .lines()
            .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
            .all(|l| l.contains("<origin xyz=")),
        "COM-only patch: {patch}"
    );

    let fr = &rec["friction"][PITCH];
    assert_eq!(fr["accepted"], true, "{fr:#}");
    let fc = fr["fc_nm"].as_f64().unwrap();
    let fv = fr["fv"].as_f64().unwrap();
    assert!((fc - (FC + 0.01)).abs() < 0.02, "fc {fc}: {fr:#}");
    assert!((fv - FV).abs() < 0.03, "fv {fv}: {fr:#}");
    let control = std::fs::read_to_string(file_ending(out.path(), ".control.patch").unwrap())
        .expect("control patch");
    assert!(
        control.contains("fc:") && control.contains("fv:"),
        "{control}"
    );

    // Both patches apply to the captured files at their repo paths.
    let apply = TempDir::new().unwrap();
    std::fs::create_dir_all(apply.path().join("assets/urdf")).unwrap();
    std::fs::create_dir_all(apply.path().join("config")).unwrap();
    std::fs::copy(
        a.path().join("pi-marengo.urdf"),
        apply.path().join("assets/urdf/marengo.urdf"),
    )
    .unwrap();
    std::fs::copy(
        a.path().join("config/control.yaml"),
        apply.path().join("config/control.yaml"),
    )
    .unwrap();
    for suffix in [".urdf.patch", ".control.patch"] {
        let git = Command::new("git")
            .args(["apply", "--check"])
            .arg(file_ending(out.path(), suffix).unwrap())
            .current_dir(apply.path())
            .output()
            .expect("git");
        assert!(
            git.status.success(),
            "{suffix}: {}",
            String::from_utf8_lossy(&git.stderr)
        );
    }
}

#[test]
fn single_wave_session_has_no_cross_session_spread_and_proposes_no_urdf_patch() {
    let a = write_session(&SESSION_A);
    let out = TempDir::new().unwrap();
    let o = run(&[a.path()], out.path());
    let stdout = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(2), "{stdout}");
    let rec = record(out.path());
    let group = &rec["groups"][0];
    assert_eq!(group["accepted"], false, "{group:#}");
    let reasons = group["refusals"].to_string();
    assert!(reasons.contains("cross-session"), "{reasons}");
    assert!(file_ending(out.path(), ".urdf.patch").is_none());
}

/// A pose seen by one bin only (one wave, one session) has no spread to judge it by: its
/// residual is reported, never gated, and the replicated poses decide the verdict.
#[test]
fn single_bin_pose_residual_is_reported_not_gated() {
    const EXTRA_POSE: f64 = 0.75;
    const ARTEFACT_NM: f64 = 0.05;
    let a = write_session(&Session {
        extra: Some((EXTRA_POSE, ARTEFACT_NM)),
        ..SESSION_A
    });
    let b = write_session(&SESSION_B);
    let out = TempDir::new().unwrap();
    let o = run(&[a.path(), b.path()], out.path());
    let rec = record(out.path());
    let group = &rec["groups"][0];
    assert_eq!(o.status.code(), Some(0), "{group:#}");
    assert_eq!(group["accepted"], true, "{group:#}");
    let gate = group["gate"]["residual_gate_nm"].as_f64().unwrap();
    let ungated: Vec<&Value> = group["poses"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["gated"] == false)
        .collect();
    // Only the extra wave's bins (those with enough samples), none of the replicated grid.
    assert!(!ungated.is_empty(), "{group:#}");
    assert!(
        ungated.iter().all(|p| {
            (p["center_rad"].as_f64().unwrap() - EXTRA_POSE).abs() <= AMPLITUDE / 2.0 + 1e-6
        }),
        "{group:#}"
    );
    let worst = group["gate"]["max_ungated_residual_nm"].as_f64().unwrap();
    assert!(
        worst > gate,
        "artefact {worst} must exceed the gate {gate}: {group:#}"
    );
    assert!(group["gate"]["max_residual_nm"].as_f64().unwrap() <= gate);
    let (a_true, _) = lumped(&truth());
    let a_fit = group["fit"]["a_nm"].as_f64().unwrap();
    assert!((a_fit - a_true).abs() < 0.02, "A {a_fit} vs {a_true}");
}

/// Friction that varies with pose (repeatable between sessions) plus a q-locked readout
/// ripple: σ_cross is tiny, but each bin's own sampling noise is not. The gate follows the
/// bin noise, and fv, unresolved over a narrow speed span, is dropped rather than
/// extrapolated (Coulomb mean only).
#[test]
fn friction_gate_follows_bin_noise_and_drops_unresolved_fv() {
    const NARROW: [f64; 3] = [0.25, 0.28, 0.31];
    let noisy = |s: &Session| Session {
        fc: FC,
        speeds: &NARROW,
        // Sample sd ≈ 0.42 Nm, inside the 0.13–0.48 Nm the 2026-10-04 pitch bins show.
        ripple_nm: 0.6,
        friction_ripple_nm: 0.05,
        ..*s
    };
    let a = write_session(&noisy(&SESSION_A));
    let b = write_session(&noisy(&SESSION_B));
    let out = TempDir::new().unwrap();
    let o = run(&[a.path(), b.path()], out.path());
    let rec = record(out.path());
    let fr = &rec["friction"][PITCH];
    assert!(o.status.code().is_some(), "{fr:#}");
    assert_eq!(fr["accepted"], true, "{fr:#}");
    let cross = fr["cross_session_sigma_nm"].as_f64().unwrap();
    let bin = fr["bin_sigma_nm"].as_f64().unwrap();
    let gate = fr["residual_gate_nm"].as_f64().unwrap();
    let worst = fr["max_residual_nm"].as_f64().unwrap();
    assert!(bin > 2.0 * cross, "bin noise {bin} vs σ_cross {cross}");
    // The pose-dependent friction exceeds a σ_cross gate but not the bin-noise gate.
    assert!(worst > 3.0 * cross && worst <= gate, "{fr:#}");
    assert!((gate - (3.0 * bin).min(0.10)).abs() < 1e-4, "{fr:#}");
    assert_eq!(fr["fv"].as_f64(), Some(0.0), "{fr:#}");
    assert!(
        fr["fv_note"].as_str().unwrap().contains("not identifiable"),
        "{fr:#}"
    );
    let fc = fr["fc_nm"].as_f64().unwrap();
    let expected = FC + FV * NARROW[1];
    assert!(
        (fc - expected).abs() < 0.04,
        "fc {fc} vs {expected}: {fr:#}"
    );
}
