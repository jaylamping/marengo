#![allow(clippy::expect_used)]

use marengo_config::MotorType;
use robstride::mit::decode_mit_feedback;
use robstride::{CanBus, CanFrame, MemoryBus, MotorAddress, MotorBus};
use std::collections::HashMap;
use std::time::Duration;

const POSE: [u8; 8] = [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8];

#[test]
fn status_decoder_requires_exactly_eight_received_bytes() {
    for id in [0x0280_01fd, 0x1880_01fd] {
        for len in 0..8 {
            assert!(decode_mit_feedback(MotorType::Rs03, id, &POSE[..len]).is_none());
        }
        let nine = [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8, 0x55];
        assert!(
            decode_mit_feedback(MotorType::Rs03, id, &nine).is_none(),
            "a nine-byte response cannot be a complete classic status"
        );
        let valid = decode_mit_feedback(MotorType::Rs03, id, &POSE).expect("eight-byte status");
        assert_eq!(valid.position_rad, 0.0);
        assert_eq!(valid.temperature_c, 20.0);
    }
}

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
        let mut suffix = Vec::new();
        let _ = bus.recv_frames(&mut suffix);
        assert!(
            !suffix.is_empty(),
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
    let mut suffix = Vec::new();
    let _ = bus.recv_frames(&mut suffix);
    assert!(
        !suffix.is_empty(),
        "unrecognized frames cannot evade the raw quota"
    );
}
