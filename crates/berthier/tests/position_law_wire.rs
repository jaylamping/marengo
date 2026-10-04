//! ADR 0039 wire gate: `control.yaml` `position_law: scaled_pd` reaches the MIT frames.
//!
//! A pitch hold at rest with one-count encoder dither (the bench's at-rest velocity dither) runs
//! through the production ControlLoop → Davout → robstride encode path. The scaled-PD law must
//! send the same kd and v_des in every frame; the legacy law (the default) toggles kd.
//! Virtual initial references are initial conditions, not device reference proof.

#![allow(clippy::expect_used)]

#[path = "support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};

use berthier::ControlLoop;
use davout::simulation::{InitialVirtualReference, SimulationBus};
use marengo_config::{load_control_config_from, write_control_config_from, PositionLaw};
use robstride::decode_mit_command_fields;

const PITCH: &str = "right_shoulder_pitch";
/// can0 motor 1 (master motors.yaml).
const PITCH_DEVICE: u32 = 1;
const TICKS: usize = 300;

fn source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn select_law(root: &Path, law: PositionLaw) {
    let dir = root.join("config");
    let mut control = load_control_config_from(&dir).expect("fixture control");
    control
        .control
        .joints
        .get_mut(PITCH)
        .expect("pitch entry")
        .position_law = law;
    write_control_config_from(&dir, &control).expect("write fixture control");
}

fn rest_hold_frames(law: Option<PositionLaw>) -> Vec<(u16, u16)> {
    let fixture = support::fixture_tree_without_diagnostics("position-law-wire", &source());
    if let Some(law) = law {
        select_law(fixture.path(), law);
    }
    let mut ctrl = ControlLoop::from_simulation(
        fixture.path(),
        SimulationBus::default(),
        InitialVirtualReference::Joints(vec![PITCH.to_string()]),
        200,
        50,
    )
    .expect("virtual initial reference");
    ctrl.supervisor_mut()
        .enable_targets(&[PITCH.to_string()])
        .expect("scoped enable");
    let queue = |ctrl: &mut ControlLoop<SimulationBus>, q: f64, dq: f64| {
        for joint in ctrl.joint_names().to_vec() {
            let (position, velocity) = if joint == PITCH { (q, dq) } else { (0.0, 0.0) };
            support::queue_joint_status(ctrl.supervisor_mut(), &joint, position, velocity);
        }
    };
    queue(&mut ctrl, 0.0, 0.0);
    ctrl.supervisor_mut().drain_feedback().expect("pose drain");
    ctrl.enter_position_hold_at(Some(PITCH), 0.0)
        .expect("home hold admitted");
    ctrl.supervisor_mut().bus_mut().clear_trace();

    // One RS03 count, toggled on an irregular pattern like a resting encoder.
    let count = f64::from(4.0 * std::f32::consts::PI) / 32767.0;
    let pattern = [0, 1, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 0];
    let mut previous = 0.0;
    for tick in 0..TICKS {
        let q = f64::from(pattern[tick % pattern.len()]) * count;
        queue(&mut ctrl, q, (q - previous) / 0.005);
        previous = q;
        ctrl.tick(None).expect("hold tick");
    }
    ctrl.supervisor()
        .bus()
        .frames()
        .iter()
        .filter(|frame| frame.id & 0xff == PITCH_DEVICE)
        .filter_map(|frame| decode_mit_command_fields(frame.id, &frame.data))
        .map(|fields| (fields.kd, fields.velocity))
        .collect()
}

fn toggles(frames: &[(u16, u16)]) -> usize {
    frames.windows(2).filter(|pair| pair[0] != pair[1]).count()
}

#[test]
fn scaled_pd_sends_constant_kd_and_v_des_at_rest_while_legacy_toggles() {
    let scaled = rest_hold_frames(Some(PositionLaw::ScaledPd));
    assert!(scaled.len() >= TICKS - 2, "one MIT frame per tick");
    assert_eq!(
        toggles(&scaled),
        0,
        "scaled-PD wire kd/v_des changed at rest"
    );
    assert_ne!(scaled[0].0, 0, "drive-side damping is on");

    // The default (no key) is the legacy law, which gates drive kd on the dithering velocity.
    let legacy = rest_hold_frames(None);
    assert!(legacy.len() >= TICKS - 2, "one MIT frame per tick");
    assert!(toggles(&legacy) > 0, "gate must reject legacy");
}
