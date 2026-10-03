#![allow(clippy::expect_used, clippy::panic)]

//! RS03 MIT velocity full scale (±20 rad/s) against a real bench capture.
//!
//! Literal type-2 status frames from right_shoulder_pitch (device id 1, RS03,
//! firmware 0.3.1.42, gear 1) in candump cd-20261003T145133Z, 10.615-10.656 s
//! after capture start, during the cruise of a downward ramp. The drive's own
//! velocity field must integrate to the position change it reports over the
//! same 40.45 ms. Under ±20 the ratio is 0.996; under the old ±50 it is 2.49.

use std::collections::HashMap;
use std::time::Duration;

use marengo_config::MotorType;
use robstride::{
    encode_mit, CanFrame, FeedbackEvent, MemoryBus, MitCommand, MotorAddress, MotorBus,
};

fn decode_status(data: [u8; 8]) -> robstride::MitFeedback {
    let mut bus = MemoryBus::default();
    bus.rx_queue.push(CanFrame {
        id: STATUS_ID,
        data,
        extended: true,
    });
    let types = HashMap::from([(MotorAddress::new("can0", 1), MotorType::Rs03)]);
    let report = bus.recv_feedback_report(&types, Duration::ZERO, Duration::ZERO);
    assert!(report.terminal_error.is_none());
    assert_eq!(report.observations.len(), 1);
    let FeedbackEvent::Status(feedback) = report.observations[0].event else {
        panic!("captured frame must decode as status");
    };
    feedback
}

const STATUS_ID: u32 = 0x0280_01fd;

/// (microseconds since the first frame, payload) for consecutive id1 type-2
/// frames; candump receive stamps 1791039103.785445 .. 1791039103.825899.
const RAMP: [(u32, [u8; 8]); 9] = [
    (0, [0x7e, 0x28, 0x75, 0x10, 0x7d, 0x99, 0x00, 0xe6]),
    (5_049, [0x7e, 0x12, 0x75, 0x1f, 0x7e, 0xaf, 0x00, 0xe6]),
    (10_100, [0x7d, 0xfb, 0x74, 0x9c, 0x7e, 0x71, 0x00, 0xe6]),
    (15_149, [0x7d, 0xe4, 0x75, 0x00, 0x7d, 0x99, 0x00, 0xe6]),
    (20_199, [0x7d, 0xce, 0x75, 0x91, 0x7d, 0x5a, 0x00, 0xe6]),
    (25_249, [0x7d, 0xb8, 0x75, 0xb0, 0x7e, 0x47, 0x00, 0xe6]),
    (30_355, [0x7d, 0xa1, 0x74, 0x92, 0x7e, 0x7f, 0x00, 0xe6]),
    (35_504, [0x7d, 0x8a, 0x75, 0x22, 0x7d, 0xa4, 0x00, 0xe6]),
    (40_454, [0x7d, 0x75, 0x76, 0x72, 0x7d, 0x72, 0x00, 0xe6]),
];

/// The drive's velocity field may disagree with its own position slope by at
/// most this fraction (observed 0.4% over this window; a wrong full scale
/// such as ±50 is off by 150%).
const RELATIVE_TOLERANCE: f64 = 0.10;

#[test]
fn rs03_status_velocity_matches_captured_position_slope() {
    let decoded: Vec<(f64, f64, f64)> = RAMP
        .iter()
        .map(|(micros, data)| {
            let feedback = decode_status(*data);
            assert_eq!(feedback.device_id, 1);
            (
                f64::from(*micros) * 1e-6,
                f64::from(feedback.position_rad),
                f64::from(feedback.velocity_rad_s),
            )
        })
        .collect();
    let (t0, p0, _) = decoded[0];
    let (t1, p1, _) = decoded[decoded.len() - 1];
    let position_delta = p1 - p0;
    // Decoded position delta over the window: -68.6 mrad, i.e. -1.697 rad/s.
    assert!(
        (position_delta + 0.0686).abs() < 0.001,
        "position delta {position_delta}"
    );
    let slope = position_delta / (t1 - t0);
    // Trapezoidal integral of the drive-reported velocity over the same window.
    let integrated: f64 = decoded
        .windows(2)
        .map(|pair| 0.5 * (pair[0].2 + pair[1].2) * (pair[1].0 - pair[0].0))
        .sum();
    let ratio = integrated / position_delta;
    assert!(
        (ratio - 1.0).abs() <= RELATIVE_TOLERANCE,
        "integrated velocity {integrated} rad vs position delta {position_delta} rad \
         (ratio {ratio}, position slope {slope} rad/s): RS03 velocity full scale is wrong"
    );
}

#[test]
fn rs03_cruise_velocity_encodes_to_drive_value() {
    // right_shoulder_pitch cruise cap (control.yaml) at gear 1. The capture
    // carried 0x8332 for this request, which the ±20 drive read as 0.50 rad/s.
    let request = MitCommand {
        device_id: 1,
        motor_type: MotorType::Rs03,
        position_rad: 0.0,
        velocity_rad_s: 1.25,
        kp: 0.0,
        kd: 0.0,
        torque_ff_nm: 0.0,
    };
    let (_, data) = encode_mit(&request).expect("finite command");
    let raw = u16::from_be_bytes([data[2], data[3]]);
    assert_eq!(raw, 0x87ff);
    let drive_value = (f64::from(raw) / 32767.0 - 1.0) * 20.0;
    assert!(
        (drive_value - 1.25).abs() <= 20.0 / 32767.0,
        "drive reads {drive_value} rad/s"
    );
}
