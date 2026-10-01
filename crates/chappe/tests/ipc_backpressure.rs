//! Actual absent/stalled Unix peers and observable admission/connection evidence.
#![cfg(unix)]
#![allow(clippy::expect_used)]

use chappe::ipc::{
    ForwardOutcome, IpcFanout, MAX_PAYLOAD_BYTES, QUEUE_BYTE_CAPACITY, QUEUE_ITEM_CAPACITY,
};
use std::io::Read;
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
    fanout.shutdown().expect("join fanout");
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
    fanout.shutdown().expect("join fanout");
    drop(peer);
    fixture.close().expect("remove socket");
}

#[tokio::test]
async fn reconnect_preserves_bounded_audit_order_and_latest_reserved_state() {
    let fixture = tempfile::tempdir().expect("fixture");
    let socket = fixture.path().join("ordered.sock");
    let fanout = IpcFanout::spawn_client(socket.clone(), chappe::Bus::default()).expect("fanout");
    for sequence in 0..1000_u64 {
        fanout.forward_runtime_to_gateway("robot/state", &sequence.to_le_bytes());
        fanout.forward_runtime_to_gateway("robot/audit/action", &sequence.to_le_bytes());
    }
    fanout.forward_runtime_to_gateway("robot/safety", &99_u64.to_le_bytes());
    fanout.forward_runtime_to_gateway("robot/heartbeat", &88_u64.to_le_bytes());
    assert_eq!(
        fanout.forward_runtime_to_gateway("robot/enable", &[1]),
        ForwardOutcome::Dropped
    );
    let listener = std::os::unix::net::UnixListener::bind(socket).expect("listener");
    let reader = tokio::task::spawn_blocking(move || {
        let (mut peer, _) = listener.accept().expect("accept");
        peer.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("wire deadline");
        let mut frames = Vec::new();
        for _ in 0..131 {
            let mut header = [0; 5];
            peer.read_exact(&mut header).expect("frame header");
            assert_eq!(header[0], chappe::ipc::DIRECTION_RUNTIME_TO_GATEWAY);
            let length = u32::from_le_bytes(header[1..].try_into().expect("topic length")) as usize;
            assert!(length < 128);
            let mut topic = vec![0; length];
            peer.read_exact(&mut topic).expect("topic");
            let mut length = [0; 4];
            peer.read_exact(&mut length).expect("payload length");
            assert_eq!(u32::from_le_bytes(length), 8);
            let mut payload = [0; 8];
            peer.read_exact(&mut payload).expect("payload");
            frames.push((
                String::from_utf8(topic).expect("topic utf8"),
                u64::from_le_bytes(payload),
            ));
        }
        frames
    });
    let frames = tokio::time::timeout(Duration::from_secs(10), reader)
        .await
        .expect("reader deadline")
        .expect("join reader");
    let audit: Vec<_> = frames
        .iter()
        .filter(|(topic, _)| topic == "robot/audit/action")
        .map(|(_, sequence)| *sequence)
        .collect();
    assert_eq!(
        audit,
        (0..128).collect::<Vec<u64>>(),
        "drop-new policy preserves accepted FIFO order"
    );
    for (topic, value) in [
        ("robot/state", 999_u64),
        ("robot/safety", 99),
        ("robot/heartbeat", 88),
    ] {
        assert!(frames.iter().any(|frame| frame == &(topic.into(), value)));
    }
    assert!(frames.iter().all(|(topic, _)| topic != "robot/enable"));
    fanout.shutdown().expect("join transport");
    fixture.close().expect("remove joined fixture");
}

fn read_peer_frame(peer: &mut std::os::unix::net::UnixStream) -> (String, Vec<u8>) {
    let mut header = [0; 5];
    peer.read_exact(&mut header).expect("wire header");
    assert_eq!(header[0], chappe::ipc::DIRECTION_RUNTIME_TO_GATEWAY);
    let length = u32::from_le_bytes(header[1..].try_into().expect("topic length")) as usize;
    assert!(length < 128);
    let mut topic = vec![0; length];
    peer.read_exact(&mut topic).expect("topic bytes");
    let mut length = [0; 4];
    peer.read_exact(&mut length).expect("payload length");
    let length = u32::from_le_bytes(length) as usize;
    assert!(length <= MAX_PAYLOAD_BYTES);
    let mut payload = vec![0; length];
    peer.read_exact(&mut payload).expect("payload bytes");
    (String::from_utf8(topic).expect("topic utf8"), payload)
}

#[tokio::test]
async fn stalled_connection_reconnects_with_latest_state_and_pending_audit_suffix() {
    let fixture = tempfile::tempdir().expect("fixture");
    let path = fixture.path().join("reconnect.sock");
    let fanout = IpcFanout::spawn_client(path.clone(), chappe::Bus::default()).expect("fanout");
    let mut changes = fanout.subscribe_connection();
    let listener = std::os::unix::net::UnixListener::bind(&path).expect("first listener");
    let (peer, listener) =
        tokio::task::spawn_blocking(move || (listener.accept().expect("first peer").0, listener))
            .await
            .expect("accept task");
    assert!(tokio::time::timeout(Duration::from_secs(5), changes.recv())
        .await
        .expect("connect deadline")
        .expect("connect"));
    // Remove only our temporary socket name so reconnect cannot drain the pending
    // FIFO before the controlled replacement listener is ready.
    std::fs::remove_file(&path).expect("disconnect reconnect address");
    let mut accepted = Vec::new();
    for sequence in 0..1000_u64 {
        let mut payload = vec![0; MAX_PAYLOAD_BYTES];
        payload[..8].copy_from_slice(&sequence.to_le_bytes());
        if fanout.forward_runtime_to_gateway("robot/audit/action", &payload)
            == ForwardOutcome::Accepted
        {
            accepted.push(sequence);
        }
    }
    assert!(
        !tokio::time::timeout(Duration::from_secs(5), changes.recv())
            .await
            .expect("stall write deadline")
            .expect("disconnect")
    );
    let before = fanout.queue_stats();
    assert_eq!(
        before.write_failures, 1,
        "failed in-flight frame is uncertain, never replayed"
    );
    let pending = before.queued_items;
    assert!(pending > 0 && pending <= 8);
    let expected = accepted[accepted.len() - pending..].to_vec();
    drop(peer);
    drop(listener);
    for sequence in 0..1000_u64 {
        fanout.forward_runtime_to_gateway("robot/state", &sequence.to_le_bytes());
    }
    fanout.forward_runtime_to_gateway("robot/safety", &88_u64.to_le_bytes());
    fanout.forward_runtime_to_gateway("robot/heartbeat", &99_u64.to_le_bytes());
    let replacement = std::os::unix::net::UnixListener::bind(path).expect("replacement listener");
    let read = tokio::task::spawn_blocking(move || {
        let (mut peer, _) = replacement.accept().expect("reconnected peer");
        peer.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read deadline");
        (0..pending + 3)
            .map(|_| read_peer_frame(&mut peer))
            .collect::<Vec<_>>()
    });
    let frames = tokio::time::timeout(Duration::from_secs(10), read)
        .await
        .expect("reconnect read deadline")
        .expect("join reader");
    let audit: Vec<_> = frames
        .iter()
        .filter(|(topic, _)| topic == "robot/audit/action")
        .map(|(_, payload)| {
            assert_eq!(payload.len(), MAX_PAYLOAD_BYTES);
            u64::from_le_bytes(payload[..8].try_into().expect("audit sequence"))
        })
        .collect();
    assert_eq!(
        audit, expected,
        "pending accepted audit suffix remains ordered"
    );
    for (topic, expected) in [
        ("robot/state", 999_u64),
        ("robot/safety", 88),
        ("robot/heartbeat", 99),
    ] {
        let payload = &frames
            .iter()
            .find(|(name, _)| name == topic)
            .expect("latest reserved frame")
            .1;
        assert_eq!(payload.as_slice(), expected.to_le_bytes());
    }
    fanout.shutdown().expect("join all runtime transport tasks");
    fixture.close().expect("remove joined fixture");
}
