//! `marengo-log-cli gravity-fit` end to end on a synthetic `pi_gravity_calibrate` session:
//! live config + URDF, a position trace generated from a perturbed model with
//! approach-dependent friction and transients.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

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
const TS: &str = "20261003T120000Z";
const POSES: [f64; 5] = [0.0, 0.25, 0.48, 0.8, 1.2];
const DELTA: f64 = 0.05;
const FRICTION_NM: f64 = 0.08;
const HEADER: &str = "tick,t_ms,joint,q,dq,q_traj,dq_traj,q_des,target,target_raw,q_env_lo,q_env_hi,lead,lead_sat,settle_error,phase,friction_mode,tau_p,tau_g,tau_f,tau_d,tau_ff_cmd,tau_meas,dq_mit,kp,kd,joint_stuck,planner_frozen,retarget_age_ms,planner_event";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn joints() -> Vec<String> {
    marengo_config::load_robot_config_from(repo().join("config"))
        .expect("robot.yaml")
        .robot
        .joints
}

fn truth() -> UrdfGravityModel {
    let model = gravity_model_from_urdf(repo().join("assets/urdf/marengo.urdf"), &joints())
        .expect("live URDF");
    let mut out = model.clone();
    for (link, scale) in [(UPPER_ARM, 1.15), (FOREARM, 1.2)] {
        let cad = model.link_inertial(link).expect("link");
        out = out
            .with_link_inertial(
                link,
                LinkInertial {
                    mass_kg: cad.mass_kg * scale,
                    com_m: cad.com_m,
                },
            )
            .expect("perturb");
    }
    out
}

/// `(target, measure, pose_index, approach)` per contract step.
fn steps(poses: &[f64]) -> Vec<(f64, Option<(usize, &'static str)>)> {
    let mut out = vec![(poses[0] - DELTA, None)];
    out.extend(
        poses
            .iter()
            .enumerate()
            .map(|(i, &p)| (p, Some((i, "below")))),
    );
    out.push((poses[poses.len() - 1] + DELTA, None));
    out.extend(
        poses
            .iter()
            .enumerate()
            .rev()
            .map(|(i, &p)| (p, Some((i, "above")))),
    );
    out
}

fn write_session(poses: &[f64]) -> TempDir {
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
    let plan_steps: Vec<Value> = steps(poses)
        .iter()
        .map(|(t, m)| match m {
            Some((i, a)) => json!({"joint": PITCH, "target_rad": t, "measure": true, "pose_index": i, "approach": a}),
            None => json!({"joint": PITCH, "target_rad": t, "measure": false}),
        })
        .collect();
    let plan = json!({
        "version": 1,
        "created_utc": "2026-10-03T12:00:00.000Z",
        "session_ts": TS,
        "profile": "arm_attached",
        "sweep_joint": PITCH,
        "fixed_rad": {},
        "poses_rad": poses,
        "approach_offset_rad": DELTA,
        "settle_sec": 2.5,
        "measure_sec": 1.5,
        "steps": plan_steps,
        "gravity_gate_report": "gravity gate (basis=hanging_rest): test\nSKIPPED gravity_model_mismatch",
    });
    std::fs::write(
        d.join("plan.json"),
        serde_json::to_string_pretty(&plan).unwrap(),
    )
    .unwrap();

    let truth = truth();
    let joints = joints();
    let pitch = joints.iter().position(|j| j == PITCH).unwrap();
    let mut csv = format!("{HEADER}\n");
    let mut tick = 0u64;
    let mut prev_target = 0.0;
    for (target, _) in steps(poses) {
        let dir = if target > prev_target { 1.0 } else { -1.0 };
        // 0.6 s moving, then 3.4 s settled short of target on the approach side.
        for k in 0..200 {
            let moving = k < 30;
            let mut q = vec![0.0; joints.len()];
            q[pitch] = if moving {
                prev_target + (target - prev_target) * f64::from(k) / 30.0
            } else {
                target - dir * 0.004
            };
            let tau = truth.gravity_torques(&q).unwrap();
            for (j, name) in joints.iter().enumerate() {
                let target_j = if j == pitch { target } else { 0.0 };
                let dq = if moving && j == pitch { 0.4 } else { 0.0 };
                let meas = tau[j] + if moving { 0.5 } else { dir * FRICTION_NM };
                let _ = writeln!(
                    csv,
                    "{tick},{},{name},{:.6},{dq:.6},0,0,0,{target_j:.6},{target_j:.6},0,0,0,0,0,Hold,static,0.05,{:.6},0,0,{:.6},{meas:.6},0,18,3,0,0,0,tick",
                    tick * 5,
                    q[j],
                    tau[j],
                    meas - 0.05,
                );
            }
            tick += 4;
        }
        prev_target = target;
    }
    std::fs::write(d.join("position-trace.csv"), csv).unwrap();
    dir
}

fn run(session: &Path, out: &Path, extra: &[&str]) -> std::process::Output {
    Command::cargo_bin("marengo-log-cli")
        .unwrap()
        .arg("gravity-fit")
        .arg("--dir")
        .arg(session)
        .arg("--out-dir")
        .arg(out)
        .arg("--repo-urdf")
        .arg(repo().join("assets/urdf/marengo.urdf"))
        .args(extra)
        .output()
        .unwrap()
}

fn record(out: &Path) -> Value {
    let path = out.join(format!("2026-10-03-gravity-{TS}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).expect("record json")).unwrap()
}

#[test]
fn proposes_mass_patch_that_applies_to_the_pi_urdf() {
    let session = write_session(&POSES);
    let out = TempDir::new().unwrap();
    let o = run(session.path(), out.path(), &[]);
    let stdout = String::from_utf8_lossy(&o.stdout);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{stdout}\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(stdout.contains("Nothing applied"), "{stdout}");

    let rec = record(out.path());
    assert_eq!(rec["accepted"], true, "{rec:#}");
    for (param, want) in [
        (format!("mass:{UPPER_ARM}"), 1.15),
        (format!("mass:{FOREARM}"), 1.2),
    ] {
        let got = rec["params"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["param"] == param.as_str())
            .unwrap_or_else(|| panic!("{param} missing: {rec:#}"))["fitted"]
            .as_f64()
            .unwrap();
        assert!((got - want).abs() < 0.03, "{param}: {got} vs {want}");
    }
    assert!(
        rec["residual_nm"]["max_abs_after"].as_f64().unwrap() < 0.02,
        "{rec:#}"
    );
    assert!(
        rec["residual_nm"]["max_abs_before"].as_f64().unwrap() > 0.1,
        "{rec:#}"
    );
    let friction = rec["poses"][2]["friction_half_diff"][0].as_f64().unwrap();
    assert!(
        (friction - FRICTION_NM).abs() < 0.02,
        "friction estimate {friction}"
    );
    assert_eq!(rec["local_urdf_matches_pi"], true);
    assert!(out
        .path()
        .join(format!("2026-10-03-gravity-{TS}.md"))
        .exists());

    let patch_path = out
        .path()
        .join(format!("2026-10-03-gravity-{TS}.urdf.patch"));
    let patch = std::fs::read_to_string(&patch_path).expect("patch");
    let changed: Vec<&str> = patch
        .lines()
        .filter(|l| {
            (l.starts_with('-') || l.starts_with('+'))
                && !l.starts_with("---")
                && !l.starts_with("+++")
        })
        .collect();
    assert_eq!(changed.len(), 4, "{patch}");
    assert!(
        changed.iter().all(|l| l.contains("<mass value=")),
        "mass-only fit: {patch}"
    );

    // The patch applies to the Pi URDF at the repo path.
    let apply = TempDir::new().unwrap();
    std::fs::create_dir_all(apply.path().join("assets/urdf")).unwrap();
    std::fs::copy(
        session.path().join("pi-marengo.urdf"),
        apply.path().join("assets/urdf/marengo.urdf"),
    )
    .unwrap();
    let git = Command::new("git")
        .args(["apply", "--check"])
        .arg(&patch_path)
        .current_dir(apply.path())
        .output()
        .expect("git");
    assert!(
        git.status.success(),
        "{}",
        String::from_utf8_lossy(&git.stderr)
    );
}

#[test]
fn refuses_ill_conditioned_parameters_without_patch() {
    let session = write_session(&POSES);
    let out = TempDir::new().unwrap();
    let o = run(
        session.path(),
        out.path(),
        &[
            "--fit",
            &format!("mass:{UPPER_ARM}"),
            "--fit",
            &format!("com:{UPPER_ARM}"),
        ],
    );
    assert_eq!(
        o.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    let rec = record(out.path());
    assert_eq!(rec["accepted"], false);
    assert!(
        rec["verdict"].as_str().unwrap().contains("ill-conditioned"),
        "{rec:#}"
    );
    assert!(rec["patch"].is_null());
    assert!(!out
        .path()
        .join(format!("2026-10-03-gravity-{TS}.urdf.patch"))
        .exists());
}

#[test]
fn refuses_poses_outside_soft_limits() {
    let session = write_session(&[0.0, 0.48, 2.95]);
    let out = TempDir::new().unwrap();
    let o = run(session.path(), out.path(), &[]);
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("refused") && stderr.contains(PITCH),
        "{stderr}"
    );
    assert!(
        std::fs::read_dir(out.path()).unwrap().next().is_none(),
        "no record"
    );
}

#[test]
fn refuses_links_outside_the_right_arm() {
    let session = write_session(&POSES);
    let out = TempDir::new().unwrap();
    let o = run(session.path(), out.path(), &["--fit", "mass:base_link"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("only right_arm links"));
}
