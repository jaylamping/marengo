//! Kernel receive times through the actual SocketCAN backend: a delayed read
//! must report when the kernel received a frame, not when the host read it.

#![cfg(all(feature = "socketcan", target_os = "linux"))]
#![allow(clippy::expect_used)]

use std::collections::HashMap;
use std::thread::sleep;
use std::time::{Duration, Instant};

use marengo_config::MotorType;
use robstride::{CanBus, EchoedCommand, MotorAddress, MotorBus, SocketCanBus};
use socketcan::{CanFrame, CanSocket, EmbeddedFrame, ExtendedId, Socket};

const POSE: [u8; 8] = [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8];
/// Longer than `comm_watchdog_ms` (100 ms): the stall the stamps must expose.
const READ_DELAY: Duration = Duration::from_millis(150);
/// Host sampling slack between the kernel stamp and the test's own instant.
const SLACK: Duration = Duration::from_millis(5);

fn motor_types() -> HashMap<MotorAddress, MotorType> {
    HashMap::from([(MotorAddress::new("vcan0", 1), MotorType::Rs03)])
}

/// The kernel may enable its global RX-stamp switch from a deferred work item;
/// a frame queued before then is stamped at the read instead.
fn open_settled() -> SocketCanBus {
    let bus = SocketCanBus::open("vcan0").expect("virtual receiver");
    sleep(Duration::from_millis(20));
    bus
}

#[test]
#[ignore = "requires virtual vcan0; no physical robot; run tests serially"]
fn delayed_read_reports_kernel_receive_time_for_drive_frames() {
    let mut bus = open_settled();
    let sender = CanSocket::open("vcan0").expect("virtual producer");
    let frame = CanFrame::new(ExtendedId::new(0x0280_01fd).expect("literal ID"), &POSE)
        .expect("classic payload");
    sender.write_frame(&frame).expect("literal status frame");
    let written = Instant::now();
    sleep(READ_DELAY);
    let read_started = Instant::now();
    let report = bus.recv_feedback_report(
        &motor_types(),
        Duration::from_millis(50),
        Duration::from_millis(1),
    );
    assert!(report.terminal_error.is_none(), "receive failed");
    assert_eq!(report.observations.len(), 1);
    let received_at = report.observations[0].received_at;
    assert!(
        received_at <= written + SLACK,
        "received_at follows the read, not the kernel receive"
    );
    assert!(read_started.duration_since(received_at) >= READ_DELAY - SLACK);
    let counts = bus.rx_timestamp_counts();
    assert_eq!(counts.anomalies(), 0, "{counts:?}");
    assert!(counts.kernel + counts.floored >= 1, "{counts:?}");
}

#[test]
#[ignore = "requires virtual vcan0; no physical robot; run tests serially"]
fn delayed_read_reports_kernel_time_for_own_transmission_echo() {
    let mut bus = open_settled();
    let (id, data) = robstride::encode_default_enable(1);
    let before_write = Instant::now();
    bus.send_frame(&robstride::CanFrame {
        id,
        data,
        extended: true,
    })
    .expect("write on receiver socket");
    let written = Instant::now();
    sleep(READ_DELAY);
    let read_started = Instant::now();
    let report = bus.recv_feedback_report(
        &motor_types(),
        Duration::from_millis(50),
        Duration::from_millis(1),
    );
    assert!(report.terminal_error.is_none(), "receive failed");
    assert_eq!(report.host_echoes.len(), 1);
    assert_eq!(report.host_echoes[0].command, EchoedCommand::Enable);
    let echoed_at = report.host_echoes[0].received_at;
    // vcan loops the echo back inside the write; never before the floor.
    assert!(echoed_at >= before_write);
    assert!(
        echoed_at <= written + SLACK,
        "echo stamped at read, not at the kernel loopback"
    );
    assert!(read_started.duration_since(echoed_at) >= READ_DELAY - SLACK);
    assert_eq!(bus.rx_timestamp_counts().anomalies(), 0);
}

#[test]
#[ignore = "requires virtual vcan0; no physical robot; run tests serially"]
fn kernel_stamps_never_exceed_read_and_keep_fifo_order() {
    let mut bus = open_settled();
    let sender = CanSocket::open("vcan0").expect("virtual producer");
    let frame = CanFrame::new(ExtendedId::new(0x0280_01fd).expect("literal ID"), &POSE)
        .expect("classic payload");
    for _ in 0..3 {
        sender.write_frame(&frame).expect("literal status frame");
        sleep(Duration::from_millis(20));
    }
    let report = bus.recv_feedback_report(
        &motor_types(),
        Duration::from_millis(50),
        Duration::from_millis(1),
    );
    let read_finished = Instant::now();
    assert_eq!(report.observations.len(), 3);
    let stamps: Vec<_> = report.observations.iter().map(|o| o.received_at).collect();
    assert!(stamps.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(stamps.iter().all(|at| *at <= read_finished));
    // Frames 20 ms apart on the wire stay roughly 20 ms apart after a batched read.
    let spread = stamps[2].duration_since(stamps[0]);
    assert!(spread >= Duration::from_millis(30), "{spread:?}");
}
