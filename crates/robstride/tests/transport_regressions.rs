#![allow(clippy::expect_used)]

use marengo_config::MotorType;
use robstride::{CanFrame, MemoryBus, MotorAddress, MotorBus};
use std::collections::HashMap;
use std::time::Duration;

const POSE: [u8; 8] = [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8];

fn queued_frame(id: u32) -> CanFrame {
    CanFrame {
        id,
        data: POSE,
        extended: true,
    }
}

#[test]
fn one_receive_poll_cannot_consume_an_entire_large_backlog() {
    let types = HashMap::from([(MotorAddress::new("can0", 1), MotorType::Rs03)]);
    for budget in [Duration::ZERO, Duration::from_millis(3)] {
        let mut bus = MemoryBus::default();
        bus.rx_queue
            .extend((0..257).map(|_| queued_frame(0x0280_01fd)));
        let report = bus.recv_feedback_report(&types, budget, Duration::ZERO);
        assert!(
            report.observations.len() <= 64,
            "one poll consumed {} responses instead of its finite quota",
            report.observations.len()
        );
        assert!(
            !bus.rx_queue.is_empty(),
            "a bounded poll must retain unread traffic"
        );
    }
}

#[test]
fn unknown_identifiers_also_consume_the_raw_receive_quota() {
    let types = HashMap::from([(MotorAddress::new("can0", 1), MotorType::Rs03)]);
    let mut bus = MemoryBus::default();
    bus.rx_queue
        .extend((0..257).map(|_| queued_frame(0x1f00_01fd)));
    let report = bus.recv_feedback_report(&types, Duration::ZERO, Duration::ZERO);
    assert!(report.observations.is_empty());
    assert!(
        !bus.rx_queue.is_empty(),
        "unrecognized frames cannot evade the raw quota"
    );
}
