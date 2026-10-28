//! A batch the bus refuses after admission latches and stops (P2C-02), and
//! every bus error is classified explicitly (P2C-04).
#![allow(clippy::expect_used)]

use std::path::PathBuf;

use robstride::{BusError, CommandError, CommandField};

use crate::simulation::{InitialVirtualReference, SimulationBus};
use crate::{DavoutError, FaultClass, Supervisor};

fn supervisor() -> Supervisor<SimulationBus> {
    Supervisor::from_simulation(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
    )
    .expect("supervisor")
}

#[test]
fn a_bus_refusal_after_admission_latches_a_controller_fault() {
    let refusals = [
        BusError::InvalidCommand(CommandError::OutOfRange {
            device_id: 1,
            field: CommandField::Velocity,
        }),
        BusError::DuplicateCommand { device_id: 1 },
        BusError::DuplicateMotorAddress {
            interface: "can0".into(),
            device_id: 1,
        },
    ];
    for refusal in refusals {
        let mut sup = supervisor();
        let error = DavoutError::Bus(refusal);
        sup.stop_after_runtime_error(&error);
        assert!(sup.has_latched_fault(), "{error} left Davout unlatched");
        let class = sup.fault_authority.first_fault().map(|fault| fault.class);
        assert_eq!(class, Some(FaultClass::Controller), "{error}");
    }
}

#[test]
fn an_unknown_joint_from_the_bus_latches_a_controller_fault() {
    let mut sup = supervisor();
    let error = DavoutError::Bus(BusError::UnknownJoint {
        joint: "ghost".into(),
    });
    sup.stop_after_runtime_error(&error);
    let class = sup.fault_authority.first_fault().map(|fault| fault.class);
    assert_eq!(class, Some(FaultClass::Controller), "{error}");
}

#[test]
fn an_empty_receive_poll_does_not_latch() {
    let mut sup = supervisor();
    sup.stop_after_runtime_error(&DavoutError::Bus(BusError::RecvTimeout));
    assert!(!sup.has_latched_fault());
}
