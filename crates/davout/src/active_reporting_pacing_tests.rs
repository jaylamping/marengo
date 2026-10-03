//! Reporting refresh timing through the actual state machine and encoded bus writes.
#![allow(clippy::expect_used)]

use std::collections::HashMap;
use std::time::{Duration, Instant};

use marengo_config::{MotorBenchLimits, MotorEntry, MotorType, MotorsConfigFile};
use robstride::bus::{BusError, CanBus, CanFrame, MemoryBus, MotorBus};
use robstride::receive::ReceiveAttempt;
use robstride::{unpack_ext_id, CommunicationType};

use crate::ActiveReportingState;

fn motor(device_id: u8, can_interface: &str) -> MotorEntry {
    MotorEntry {
        joint: format!("j{device_id}"),
        driver: "robstride".into(),
        motor_type: MotorType::Rs02,
        can_interface: can_interface.into(),
        device_id,
        direction: 1,
        gear_ratio: 1.0,
        recv_can_id: 0,
        firmware_version: "test".into(),
        bench: MotorBenchLimits {
            position_lower_rad: -1.0,
            position_upper_rad: 1.0,
            velocity_limit_rad_s: 1.0,
            torque_limit_nm: 1.0,
        },
    }
}

fn motors() -> MotorsConfigFile {
    MotorsConfigFile {
        motors: (1..=5).map(|device_id| motor(device_id, "can0")).collect(),
    }
}

/// One sync per control period until every initial On is written.
fn settle_on<B: MotorBus>(
    state: &mut ActiveReportingState,
    bus: &mut B,
    motors: &MotorsConfigFile,
    start: Instant,
) -> Instant {
    let mut now = start;
    for _ in 0..motors.motors.len() {
        state.sync(bus, motors, false, true, now, &feedback(now));
        now += SPACING;
    }
    assert!(motors
        .motors
        .iter()
        .all(|motor| state.applied_on(&motor.joint)));
    now
}

const SPACING: Duration = Duration::from_millis(5);

fn feedback(now: Instant) -> HashMap<String, Instant> {
    (1..=5).map(|id| (format!("j{id}"), now)).collect()
}

fn commands(bus: &MemoryBus) -> Vec<(u8, u8)> {
    bus.tx
        .iter()
        .map(|frame| {
            let header = unpack_ext_id(frame.id).expect("encoded reporting ID");
            assert!(frame.extended);
            assert_eq!(header.comm_type, CommunicationType::ActiveReporting.as_u8());
            assert_eq!(frame.data, [1, 2, 3, 4, 5, 6, frame.data[6], 0]);
            (header.device_id, frame.data[6])
        })
        .collect()
}

#[test]
fn initial_ons_and_offs_take_one_slot_per_interface_per_period() {
    // 2026-10-03: five type-24 Ons in one call (each solicits a reply and
    // starts a 100 Hz stream) overran the mcp251x two-frame receive buffer.
    let motors = motors();
    let mut state = ActiveReportingState::default();
    let mut bus = MemoryBus::default();
    let start = Instant::now();
    for id in 1..=5u8 {
        let now = start + SPACING * u32::from(id - 1);
        state.sync(&mut bus, &motors, false, true, now, &feedback(now));
        assert_eq!(commands(&bus), vec![(id, 1)], "one On per period");
        bus.tx.clear();
        state.sync(&mut bus, &motors, false, true, now, &feedback(now));
        let almost = now + SPACING - Duration::from_micros(1);
        state.sync(&mut bus, &motors, false, true, almost, &feedback(now));
        assert!(
            bus.tx.is_empty(),
            "repeated callers cannot recreate a burst"
        );
    }

    // Activation turns every stream Off, also one per period.
    let active = start + SPACING * 5;
    for id in 1..=5u8 {
        let now = active + SPACING * u32::from(id - 1);
        state.sync(&mut bus, &motors, true, true, now, &feedback(now));
        state.sync(&mut bus, &motors, true, true, now, &feedback(now));
        assert_eq!(commands(&bus), vec![(id, 0)], "one Off per period");
        bus.tx.clear();
    }
    assert!((1..=5).all(|id| !state.applied_on(&format!("j{id}"))));
}

#[test]
fn interfaces_have_independent_slots() {
    let motors = MotorsConfigFile {
        motors: vec![
            motor(1, "can0"),
            motor(2, "can0"),
            motor(3, "can1"),
            motor(4, "can1"),
        ],
    };
    let mut state = ActiveReportingState::default();
    let mut bus = MemoryBus::default();
    let start = Instant::now();
    state.sync(&mut bus, &motors, false, true, start, &feedback(start));
    assert_eq!(commands(&bus), vec![(1, 1), (3, 1)]);
    bus.tx.clear();
    let next = start + SPACING;
    state.sync(&mut bus, &motors, false, true, next, &feedback(next));
    assert_eq!(commands(&bus), vec![(2, 1), (4, 1)]);
}

#[test]
fn healthy_refreshes_are_spaced_across_joints_and_repeated_syncs() {
    let motors = motors();
    let mut state = ActiveReportingState::default();
    let mut bus = MemoryBus::default();
    let start = settle_on(&mut state, &mut bus, &motors, Instant::now());
    bus.tx.clear();
    for cycle in 1..=2 {
        for id in 1..=5 {
            let now = start + Duration::from_secs(cycle) + Duration::from_millis((id - 1) * 5);
            let rx = feedback(now);
            state.sync(&mut bus, &motors, false, true, now, &rx);
            assert_eq!(commands(&bus), vec![(id as u8, 1)], "one healthy refresh");
            bus.tx.clear();
            state.sync(&mut bus, &motors, false, true, now, &rx);
            state.sync(
                &mut bus,
                &motors,
                false,
                true,
                now + Duration::from_millis(4),
                &rx,
            );
            assert!(
                bus.tx.is_empty(),
                "repeated callers cannot recreate a burst"
            );
        }
    }
}

#[derive(Default)]
struct FailingRefreshBus {
    memory: MemoryBus,
    failing_id: Option<u8>,
}

impl CanBus for FailingRefreshBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.memory.send_frame(frame)?;
        let header = unpack_ext_id(frame.id).expect("encoded reporting ID");
        if self.failing_id == Some(header.device_id) && frame.data[6] == 1 {
            return Err(BusError::Send {
                message: "scripted reporting failure".into(),
            });
        }
        Ok(())
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        self.memory.recv_one_nonblocking()
    }
}

impl MotorBus for FailingRefreshBus {}

#[test]
fn failed_healthy_refresh_does_not_starve_peers_or_spin() {
    let motors = motors();
    let mut state = ActiveReportingState::default();
    let mut bus = FailingRefreshBus::default();
    let start = settle_on(&mut state, &mut bus, &motors, Instant::now());
    bus.memory.tx.clear();
    bus.failing_id = Some(1);

    for (offset, id) in [1, 2, 3, 4, 5, 1].into_iter().enumerate() {
        let now = start + Duration::from_secs(1) + Duration::from_millis(offset as u64 * 5);
        let rx = feedback(now);
        state.sync(&mut bus, &motors, false, true, now, &rx);
        assert_eq!(commands(&bus.memory), vec![(id, 1)], "peers precede retry");
        bus.memory.tx.clear();
        state.sync(&mut bus, &motors, false, true, now, &rx);
        assert!(
            bus.memory.tx.is_empty(),
            "failed attempts also consume spacing"
        );
    }
}

#[test]
fn stale_retry_and_off_take_the_slot_before_a_healthy_refresh() {
    let motors = motors();
    let mut state = ActiveReportingState::default();
    let mut bus = MemoryBus::default();
    let start = settle_on(&mut state, &mut bus, &motors, Instant::now());
    bus.tx.clear();
    // j1's refresh is due; j2 has gone stale, so its retry goes first.
    let now = start + Duration::from_secs(1);
    let mut rx = feedback(now);
    rx.insert("j2".into(), now - Duration::from_millis(201));
    state.sync(&mut bus, &motors, false, true, now, &rx);
    assert_eq!(
        commands(&bus),
        vec![(2, 1)],
        "stale recovery precedes refresh"
    );
    bus.tx.clear();
    let next = now + SPACING;
    let rx = feedback(next);
    state.sync(&mut bus, &motors, false, true, next, &rx);
    assert_eq!(commands(&bus), vec![(1, 1)], "deferred refresh follows");
    bus.tx.clear();

    // Activation while j3's refresh would be due: Off wins the slot.
    let active = next + SPACING;
    state.sync(&mut bus, &motors, true, true, active, &feedback(active));
    assert_eq!(commands(&bus), vec![(1, 0)]);
}
