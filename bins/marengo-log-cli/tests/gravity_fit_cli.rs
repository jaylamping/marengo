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
const HEADER: &str = "tick,t_ms,joint,q,dq,q_traj,dq_traj,q_des,target,target_raw,q_env_lo,q_env_hi,lead,lead_sat,settle_error,phase,friction_mode,tau_p,tau_g,tau_f,tau_d,tau_ff_cmd,tau_meas,dq_mit,kp,kd,joint_stuck,planner_frozen,retarget_age_ms,planner_event,law,q_ref,dq_ref,time_scale,tau_i,kd_mit,tau_ff_wire";

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
                // At rest, drive velocity feedback dithers by one count (~0.077 rad/s).
                let dq = if moving && j == pitch {
                    0.4
                } else if !moving && (k as usize + j) % 3 == 0 {
                    0.077
                } else {
                    0.0
                };
                let meas = tau[j] + if moving { 0.5 } else { dir * FRICTION_NM };
                let _ = writeln!(
                    csv,
                    "{tick},{},{name},{:.6},{dq:.6},0,0,0,{target_j:.6},{target_j:.6},0,0,0,0,0,Hold,static,0.05,{:.6},0,0,{:.6},{meas:.6},0,18,3,0,0,0,tick,legacy,0.000000,0.000000,1.000000,0.000000,3.000,{:.6}",
                    tick * 5,
                    q[j],
                    tau[j],
                    meas - 0.05,
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
    let captured_urdf =
        std::fs::read(session.path().join("pi-marengo.urdf")).expect("captured URDF");
    let out = TempDir::new().expect("out");
    let o = run(session.path(), out.path(), &[]);
    let stdout = String::from_utf8_lossy(&o.stdout);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{stdout}\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(stdout.contains("Nothing applied"), "{stdout}");
    assert!(
        !session.path().join("proposed-marengo.urdf").exists(),
        "gravity-fit must not write proposed URDF into captured evidence"
    );
    assert_eq!(
        std::fs::read(session.path().join("pi-marengo.urdf")).expect("captured URDF unchanged"),
        captured_urdf,
        "gravity-fit must leave captured evidence byte-identical"
    );
    assert!(
        out.path()
            .join(format!("2026-10-03-gravity-{TS}.proposed-marengo.urdf"))
            .exists(),
        "proposed URDF belongs with generated output"
    );

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
fn gravity_fit_requires_explicit_output_and_local_urdf_paths() {
    let session = write_session(&POSES);
    let output = Command::cargo_bin("marengo-log-cli")
        .expect("binary")
        .args(["gravity-fit", "--dir"])
        .arg(session.path())
        .output()
        .expect("gravity-fit");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--out-dir"), "{stderr}");
    assert!(stderr.contains("--repo-urdf"), "{stderr}");
    assert!(
        !session.path().join("proposed-marengo.urdf").exists(),
        "missing paths must not make the CLI mutate evidence"
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
fn refuses_fused_sessions_with_different_calibration_windows() {
    let first = write_session(&POSES);
    let second = write_session(&POSES);
    let control_path = second.path().join("config/control.yaml");
    let control = std::fs::read_to_string(&control_path).expect("control");
    // Narrow pitch's window by 0.1 rad, whatever the master value is.
    let block = control
        .find(&format!("\n    {PITCH}:\n"))
        .expect("pitch block");
    let key = "position_soft_upper_rad: ";
    let start = block + control[block..].find(key).expect("pitch soft upper") + key.len();
    let end = start + control[start..].find('\n').expect("line end");
    let upper: f64 = control[start..end].parse().expect("pitch soft upper value");
    let changed = format!("{}{}{}", &control[..start], upper - 0.1, &control[end..]);
    std::fs::write(control_path, changed).expect("write changed control");
    let out = TempDir::new().expect("out");

    let second_dir = second.path().to_str().expect("utf-8 fixture path");
    let result = run(first.path(), out.path(), &["--dir", second_dir]);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_ne!(result.status.code(), Some(0), "{stderr}");
    assert!(
        stderr.contains("different effective calibration windows"),
        "{stderr}"
    );
    assert!(
        std::fs::read_dir(out.path())
            .expect("out dir")
            .next()
            .is_none(),
        "refusal writes no calibration result"
    );
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
// ---- right-arm calibration suite fixtures (v2 plans, partial sessions, waves) ----
//
// Synthetic sessions built from the perturbed truth model with known Coulomb/viscous
// friction. v2 plans (`kind` steps, `session_complete`) exercise the suite contract;
// truncated traces exercise partial sessions.

const ELBOW: &str = "right_elbow_pitch";
const ELBOW_POSES: [f64; 3] = [0.0, 0.25, 0.5];
const FIXED_PITCH: f64 = 0.5;
const TS_PITCH: &str = "20261004T010000Z";
const TS_ELBOW: &str = "20261004T020000Z";
const TS_WAVE: &str = "20261004T030000Z";
const TS_PARTIAL: &str = "20261004T040000Z";
const TS_MISMATCH: &str = "20261004T050000Z";

struct WaveSpec {
    min: f64,
    max: f64,
    cycles: u32,
    half_period_s: f64,
    fc: f64,
    fv: f64,
}

struct SynthOpts {
    ts: &'static str,
    sweep: &'static str,
    fixed: Vec<(&'static str, f64)>,
    poses: Vec<f64>,
    fc_hold: f64,
    wave: Option<WaveSpec>,
    keep_steps: Option<usize>,
    session_complete: bool,
}

enum SynthStep {
    Fixed {
        joint: &'static str,
        target: f64,
    },
    Hold {
        joint: &'static str,
        target: f64,
        measure: bool,
        pose: Option<usize>,
        approach: Option<&'static str>,
    },
    Wave {
        joint: &'static str,
        wave: WaveSpec,
    },
}

fn synth_plan_steps(o: &SynthOpts) -> Vec<SynthStep> {
    let mut steps = Vec::new();
    for (joint, target) in &o.fixed {
        steps.push(SynthStep::Fixed {
            joint,
            target: *target,
        });
    }
    steps.push(SynthStep::Hold {
        joint: o.sweep,
        target: o.poses[0] - DELTA,
        measure: false,
        pose: None,
        approach: None,
    });
    for (i, &p) in o.poses.iter().enumerate() {
        steps.push(SynthStep::Hold {
            joint: o.sweep,
            target: p,
            measure: true,
            pose: Some(i),
            approach: Some("below"),
        });
    }
    if let Some(w) = &o.wave {
        steps.push(SynthStep::Wave {
            joint: o.sweep,
            wave: WaveSpec {
                min: w.min,
                max: w.max,
                cycles: w.cycles,
                half_period_s: w.half_period_s,
                fc: w.fc,
                fv: w.fv,
            },
        });
    }
    steps.push(SynthStep::Hold {
        joint: o.sweep,
        target: o.poses[o.poses.len() - 1] + DELTA,
        measure: false,
        pose: None,
        approach: None,
    });
    for (i, &p) in o.poses.iter().enumerate().rev() {
        steps.push(SynthStep::Hold {
            joint: o.sweep,
            target: p,
            measure: true,
            pose: Some(i),
            approach: Some("above"),
        });
    }
    steps
}

fn write_synth(o: &SynthOpts) -> TempDir {
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
    let plan_steps: Vec<Value> = synth_plan_steps(o)
        .iter()
        .map(|s| match s {
            SynthStep::Fixed { joint, target } => {
                json!({"kind": "fixed", "joint": joint, "target_rad": target})
            }
            SynthStep::Hold {
                joint,
                target,
                measure,
                pose,
                approach,
            } => {
                let mut v = json!({"kind": "hold", "joint": joint, "target_rad": target,
                    "measure": measure});
                if let Some(i) = pose {
                    v["pose_index"] = json!(i);
                }
                if let Some(a) = approach {
                    v["approach"] = json!(a);
                }
                v
            }
            SynthStep::Wave { joint, wave } => json!({"kind": "wave", "joint": joint,
                "min_rad": wave.min, "max_rad": wave.max, "cycles": wave.cycles,
                "half_period_s": wave.half_period_s}),
        })
        .collect();
    let fixed_rad: serde_json::Map<String, Value> = o
        .fixed
        .iter()
        .map(|(j, v)| (j.to_string(), json!(v)))
        .collect();
    let plan = json!({
        "version": 2,
        "created_utc": "2026-10-04T01:00:00.000Z",
        "session_ts": o.ts,
        "session_complete": o.session_complete,
        "profile": "arm_attached",
        "sweep_joint": o.sweep,
        "fixed_rad": fixed_rad,
        "poses_rad": o.poses,
        "approach_offset_rad": DELTA,
        "settle_sec": 2.5,
        "measure_sec": 1.5,
        "steps": plan_steps,
        "gravity_gate_report": "synthetic",
    });
    std::fs::write(
        d.join("plan.json"),
        serde_json::to_string_pretty(&plan).unwrap(),
    )
    .unwrap();
    let truth = truth();
    let joints = joints();
    let index = |name: &str| joints.iter().position(|j| j == name).unwrap();
    let mut at = vec![0.0; joints.len()];
    let mut csv = format!("{HEADER}\n");
    let mut tick = 0u64;
    let emit = |tick: &mut u64,
                csv: &mut String,
                q: &[f64],
                dq: &[f64],
                tau: &[f64],
                meas: &[f64],
                targets: &[f64]| {
        for (j, name) in joints.iter().enumerate() {
            let _ = writeln!(
                csv,
                "{tick},{},{name},{:.6},{:.6},0,0,0,{:.6},{:.6},0,0,0,0,0,Hold,static,0.05,{:.6},0,0,{:.6},{:.6},0,18,3,0,0,0,tick,legacy,0.000000,0.000000,1.000000,0.000000,3.000,{:.6}",
                *tick * 5,
                q[j],
                dq[j],
                targets[j],
                targets[j],
                tau[j],
                meas[j] - 0.05,
                meas[j],
                meas[j] - 0.05,
            );
        }
        *tick += 4;
    };
    let steps = synth_plan_steps(o);
    let keep = o.keep_steps.unwrap_or(steps.len());
    for s in steps.iter().take(keep) {
        match s {
            SynthStep::Fixed { joint, target } | SynthStep::Hold { joint, target, .. } => {
                let ji = index(joint);
                let from = at[ji];
                let dir = if *target > from {
                    1.0
                } else if *target < from {
                    -1.0
                } else {
                    0.0
                };
                let mut targets = at.clone();
                targets[ji] = *target;
                for k in 0..200u32 {
                    let moving = k < 30;
                    let mut q = at.clone();
                    q[ji] = if moving {
                        from + (target - from) * f64::from(k) / 30.0
                    } else {
                        target - dir * 0.004
                    };
                    let torques = truth.gravity_torques(&q).unwrap();
                    let tau: Vec<f64> = (0..joints.len()).map(|j| torques[j]).collect();
                    let mut meas = tau.clone();
                    let mut dq = vec![0.0; joints.len()];
                    if moving {
                        dq[ji] = 0.4;
                        meas[ji] += 0.5;
                    } else {
                        if (k as usize + ji) % 3 == 0 {
                            dq[ji] = 0.077;
                        }
                        meas[ji] += dir * o.fc_hold;
                    }
                    emit(&mut tick, &mut csv, &q, &dq, &tau, &meas, &targets);
                }
                at[ji] = *target;
            }
            SynthStep::Wave { joint, wave } => {
                let ji = index(joint);
                let mid = (wave.min + wave.max) / 2.0;
                let amp = (wave.max - wave.min) / 2.0;
                // Odd offset keeps the wave target off every hold target for segmentation.
                let wave_target = mid + 0.013;
                let mut targets = at.clone();
                targets[ji] = wave_target;
                let total_s = f64::from(wave.cycles) * 2.0 * wave.half_period_s;
                let iters = (total_s / 0.02).round() as usize;
                for k in 0..iters {
                    let t = k as f64 * 0.02;
                    let phase = std::f64::consts::PI * t / wave.half_period_s;
                    let mut q = at.clone();
                    q[ji] = mid - amp * phase.cos();
                    let mut dq = vec![0.0; joints.len()];
                    dq[ji] = amp * std::f64::consts::PI / wave.half_period_s * phase.sin();
                    let torques = truth.gravity_torques(&q).unwrap();
                    let tau: Vec<f64> = (0..joints.len()).map(|j| torques[j]).collect();
                    let mut meas = tau.clone();
                    meas[ji] += wave.fc * dq[ji].signum() + wave.fv * dq[ji];
                    emit(&mut tick, &mut csv, &q, &dq, &tau, &meas, &targets);
                }
                at[ji] = wave.min;
            }
        }
    }
    std::fs::write(d.join("position-trace.csv"), csv).unwrap();
    dir
}

fn run_multi(sessions: &[&Path], out: &Path, extra: &[&str]) -> std::process::Output {
    let mut cmd = Command::cargo_bin("marengo-log-cli").unwrap();
    cmd.arg("gravity-fit");
    for s in sessions {
        cmd.arg("--dir").arg(s);
    }
    cmd.arg("--out-dir")
        .arg(out)
        .arg("--repo-urdf")
        .arg(repo().join("assets/urdf/marengo.urdf"))
        .args(extra);
    cmd.output().unwrap()
}

fn record_any(out: &Path) -> Value {
    let mut found = None;
    for e in std::fs::read_dir(out).expect("out") {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "json") {
            found = Some(p);
            break;
        }
    }
    serde_json::from_str(&std::fs::read_to_string(found.expect("record json")).unwrap()).unwrap()
}

fn fitted_param(rec: &Value, param: &str) -> f64 {
    rec["params"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["param"] == param)
        .unwrap_or_else(|| panic!("{param} missing: {rec:#}"))["fitted"]
        .as_f64()
        .unwrap()
}

#[test]
fn partial_session_fits_completed_steps() {
    // Full v2 pitch plan is 12 steps; the trace stops after 10 (3 of 5 down-pass
    // holds), so 2 poses never complete and must be skipped, not refused.
    let session = write_synth(&SynthOpts {
        ts: TS_PARTIAL,
        sweep: PITCH,
        fixed: vec![],
        poses: POSES.to_vec(),
        fc_hold: FRICTION_NM,
        wave: None,
        keep_steps: Some(10),
        session_complete: false,
    });
    let out = TempDir::new().unwrap();
    let o = run_multi(&[session.path()], out.path(), &[]);
    let stdout = String::from_utf8_lossy(&o.stdout);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{stdout}\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let rec = record_any(out.path());
    assert_eq!(rec["accepted"], true, "{rec:#}");
    assert_eq!(rec["sessions"][0]["session_complete"], false);
    let skipped = rec["sessions"][0]["skipped_steps"].as_array().unwrap();
    assert!(skipped.len() >= 2, "aborted steps reported: {rec:#}");
    for (want, want_v) in [
        (format!("mass:{UPPER_ARM}"), 1.15),
        (format!("mass:{FOREARM}"), 1.2),
    ] {
        let got = fitted_param(&rec, &want);
        assert!((got - want_v).abs() < 0.06, "{want}: {got} vs {want_v}");
    }
}

#[test]
fn fused_pitch_and_elbow_separates_arm_masses() {
    let pitch = write_synth(&SynthOpts {
        ts: TS_PITCH,
        sweep: PITCH,
        fixed: vec![],
        poses: POSES.to_vec(),
        fc_hold: FRICTION_NM,
        wave: None,
        keep_steps: None,
        session_complete: true,
    });
    let elbow = write_synth(&SynthOpts {
        ts: TS_ELBOW,
        sweep: ELBOW,
        fixed: vec![(PITCH, FIXED_PITCH)],
        poses: ELBOW_POSES.to_vec(),
        fc_hold: FRICTION_NM,
        wave: None,
        keep_steps: None,
        session_complete: true,
    });
    let out = TempDir::new().unwrap();
    let o = run_multi(&[pitch.path(), elbow.path()], out.path(), &[]);
    let stdout = String::from_utf8_lossy(&o.stdout);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{stdout}\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let rec = record_any(out.path());
    assert_eq!(rec["accepted"], true, "{rec:#}");
    assert_eq!(rec["sessions"].as_array().unwrap().len(), 2);
    assert!((fitted_param(&rec, &format!("mass:{UPPER_ARM}")) - 1.15).abs() < 0.05);
    assert!((fitted_param(&rec, &format!("mass:{FOREARM}")) - 1.2).abs() < 0.05);
}

#[test]
fn wave_friction_fit_recovers_fc_and_fv() {
    let session = write_synth(&SynthOpts {
        ts: TS_WAVE,
        sweep: PITCH,
        fixed: vec![],
        poses: POSES.to_vec(),
        fc_hold: 0.12,
        wave: Some(WaveSpec {
            min: 0.1,
            max: 0.5,
            cycles: 2,
            half_period_s: 1.0,
            fc: 0.12,
            fv: 0.06,
        }),
        keep_steps: None,
        session_complete: true,
    });
    let out = TempDir::new().unwrap();
    let o = run_multi(&[session.path()], out.path(), &[]);
    let stdout = String::from_utf8_lossy(&o.stdout);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{stdout}\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let rec = record_any(out.path());
    assert_eq!(rec["accepted"], true, "{rec:#}");
    let fr = &rec["friction"]["joints"][PITCH];
    assert!(
        (fr["static_fc_nm"].as_f64().unwrap() - 0.12).abs() < 0.02,
        "{fr:#}"
    );
    assert!(
        (fr["wave_fc_nm"].as_f64().unwrap() - 0.12).abs() < 0.02,
        "{fr:#}"
    );
    assert!(
        (fr["wave_fv"].as_f64().unwrap() - 0.06).abs() < 0.02,
        "{fr:#}"
    );
    assert!(fr["wave_r2"].as_f64().unwrap() > 0.9, "{fr:#}");
    assert_eq!(fr["consistent"], true);
    assert!(rec["control_patch"].is_string(), "{rec:#}");
    let patches: Vec<_> = std::fs::read_dir(out.path())
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".control.patch")
                .then_some(p)
        })
        .collect();
    assert_eq!(patches.len(), 1);
    let patch = std::fs::read_to_string(&patches[0]).unwrap();
    assert!(patch.contains("fc:") && patch.contains("fv:"), "{patch}");
}

#[test]
fn inconsistent_friction_proposes_no_control_patch() {
    let session = write_synth(&SynthOpts {
        ts: TS_MISMATCH,
        sweep: PITCH,
        fixed: vec![],
        poses: POSES.to_vec(),
        fc_hold: 0.08,
        wave: Some(WaveSpec {
            min: 0.1,
            max: 0.5,
            cycles: 2,
            half_period_s: 1.0,
            fc: 0.30,
            fv: 0.06,
        }),
        keep_steps: None,
        session_complete: true,
    });
    let out = TempDir::new().unwrap();
    let o = run_multi(&[session.path()], out.path(), &[]);
    let stdout = String::from_utf8_lossy(&o.stdout);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{stdout}\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let rec = record_any(out.path());
    assert_eq!(rec["accepted"], true, "{rec:#}");
    assert_eq!(rec["friction"]["joints"][PITCH]["consistent"], false);
    assert!(rec["control_patch"].is_null(), "{rec:#}");
    assert!(
        std::fs::read_dir(out.path())
            .unwrap()
            .filter_map(|e| {
                let p = e.unwrap().path();
                p.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .ends_with(".control.patch")
                    .then_some(p)
            })
            .next()
            .is_none(),
        "inconsistent estimates must not propose a friction patch"
    );
}
