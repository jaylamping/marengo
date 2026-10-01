//! Actual absent/stalled Unix peers and observable admission/connection evidence.
#![cfg(unix)]
#![allow(clippy::expect_used)]

use chappe::ipc::{
    ForwardOutcome, IpcFanout, MAX_PAYLOAD_BYTES, QUEUE_BYTE_CAPACITY, QUEUE_ITEM_CAPACITY,
};
use std::time::Duration;

#[test]
fn absent_peer_has_bounded_classes_and_honest_counters() {
    let fixture = tempfile::tempdir().expect("fixture");
    let fanout =
        IpcFanout::spawn_client(fixture.path().join("absent.sock"), chappe::Bus::default())
            .expect("fanout");
    for index in 0..1000_u64 {
        let outcome = fanout.forward_runtime_to_gateway("robot/state", &index.to_le_bytes());
        assert_eq!(
            outcome,
            if index == 0 {
                ForwardOutcome::Accepted
            } else {
                ForwardOutcome::Coalesced
            }
        );
    }
    let payload = vec![0; MAX_PAYLOAD_BYTES];
    for _ in 0..1000 {
        fanout.forward_runtime_to_gateway("logs/structured", &payload);
    }
    assert_eq!(
        fanout.forward_runtime_to_gateway("robot/safety", &[1]),
        ForwardOutcome::Accepted
    );
    assert_eq!(
        fanout.forward_runtime_to_gateway("robot/heartbeat", &[2]),
        ForwardOutcome::Accepted
    );
    assert_eq!(
        fanout.forward_runtime_to_gateway("robot/enable", &[1]),
        ForwardOutcome::Dropped
    );
    assert_eq!(
        fanout.forward_runtime_to_gateway("logs/structured", &vec![0; MAX_PAYLOAD_BYTES + 1]),
        ForwardOutcome::Dropped
    );
    let stats = fanout.queue_stats();
    assert!(!stats.connected);
    assert_eq!(stats.queued_items, 11);
    assert_eq!(stats.queued_bytes, 8 * MAX_PAYLOAD_BYTES + 10);
    assert_eq!(
        (stats.accepted, stats.coalesced, stats.dropped),
        (11, 999, 994)
    );
    assert_eq!(stats.admitted_disconnected, 1010);
    assert!(stats.queued_items <= QUEUE_ITEM_CAPACITY && stats.queued_bytes <= QUEUE_BYTE_CAPACITY);
    fanout.shutdown();
    assert_eq!(
        fanout.forward_runtime_to_gateway("robot/state", &[3]),
        ForwardOutcome::Dropped
    );
    fixture.close().expect("remove fixture");
}

#[tokio::test]
async fn accepting_nonreading_peer_expires_write_and_bounds_admission() {
    let fixture = tempfile::tempdir().expect("fixture");
    let socket = fixture.path().join("stall.sock");
    let fanout = IpcFanout::spawn_client(socket.clone(), chappe::Bus::default()).expect("fanout");
    let mut connection = fanout.subscribe_connection();
    let listener = std::os::unix::net::UnixListener::bind(socket).expect("peer listener");
    let peer = tokio::task::spawn_blocking(move || listener.accept().expect("accept").0)
        .await
        .expect("accept task");
    assert!(
        tokio::time::timeout(Duration::from_secs(5), connection.recv())
            .await
            .expect("connect deadline")
            .expect("connection")
    );
    // Retain the accepted socket without reading. The real kernel writer must
    // exhaust its buffer; all admissions stay finite even while it is stalled.
    let payload = vec![0; MAX_PAYLOAD_BYTES];
    for _ in 0..10000 {
        fanout.forward_runtime_to_gateway("logs/structured", &payload);
        let stats = fanout.queue_stats();
        assert!(
            stats.queued_items <= QUEUE_ITEM_CAPACITY && stats.queued_bytes <= QUEUE_BYTE_CAPACITY
        );
    }
    assert!(
        !tokio::time::timeout(Duration::from_secs(5), connection.recv())
            .await
            .expect("write deadline teardown")
            .expect("disconnect")
    );
    assert!(!fanout.queue_stats().connected);
    assert!(fanout.queue_stats().dropped > 0);
    fanout.shutdown();
    drop(peer);
    fixture.close().expect("remove socket");
}
