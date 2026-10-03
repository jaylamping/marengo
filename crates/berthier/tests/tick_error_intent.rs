//! A safety error returned by `tick` always leaves the control mode Disabled.
//!
//! marengo-pi used to keep a Position hold across a `CommWatchdog` tick error
//! (a branch that assumed the loop preserved it). The loop discards motion
//! intent for every `LoopError::Safety` and for feedback-loss errors before
//! returning, so that branch was dead and a later enable could never resume a
//! stale hold (audit L-berthier-25).

#![allow(clippy::expect_used)]

use std::time::Duration;

use berthier::{ControlLoop, ControlMode, LoopError};
use davout::simulation::{InitialVirtualReference, SimulationBus, SimulationReceive};
use davout::DavoutError;
mod support;
use support::queue_all_status;

fn active_gravity_comp() -> ControlLoop<SimulationBus> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut controller = ControlLoop::from_simulation(
        root,
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
        200,
        50,
    )
    .expect("virtual initial reference fixture");
    controller
        .supervisor_mut()
        .set_homing_complete()
        .expect("ready");
    controller
        .supervisor_mut()
        .request_enable(true)
        .expect("recording enable");
    controller.set_control_mode(ControlMode::GravityComp);
    // Fresh status for every joint, so the neutral bootstrap completes and the
    // loop is Active with live feedback.
    for _ in 0..4 {
        queue_all_status(controller.supervisor_mut(), None);
        controller.tick(None).expect("tick with fresh feedback");
    }
    assert_eq!(controller.control_mode(), ControlMode::GravityComp);
    controller
}

#[test]
fn safety_tick_error_discards_motion_intent() {
    let mut controller = active_gravity_comp();
    controller
        .supervisor_mut()
        .bus_mut()
        .queue_attempts([SimulationReceive::Error("injected receive failure".into())])
        .expect("finite receive failure");
    let error = controller.tick(None).expect_err("receive failure");
    assert!(
        matches!(error, LoopError::Safety(DavoutError::Bus(_))),
        "unexpected {error}"
    );
    assert_eq!(
        controller.control_mode(),
        ControlMode::Disabled,
        "a Safety tick error must not leave a hold armed"
    );
}

#[test]
fn feedback_silence_tick_error_discards_motion_intent() {
    let mut controller = active_gravity_comp();
    let watchdog = Duration::from_millis(controller.supervisor().control.control.comm_watchdog_ms);
    std::thread::sleep(watchdog + Duration::from_millis(40));
    let error = loop {
        if let Err(error) = controller.tick(None) {
            break error;
        }
    };
    assert!(
        matches!(
            error,
            LoopError::Safety(DavoutError::CommWatchdog { .. }) | LoopError::MissingFeedback { .. }
        ),
        "unexpected {error}"
    );
    assert_eq!(controller.control_mode(), ControlMode::Disabled);
}
