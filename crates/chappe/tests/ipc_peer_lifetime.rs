//! Connection replacement and retirement through actual Unix sockets.
#![cfg(unix)]
#![allow(clippy::expect_used)]
use armee_proto::prost::Message;
use chappe::ipc::{encode_frame, IpcListener, DIRECTION_RUNTIME_TO_GATEWAY};
use std::io::{Read, Write};
use std::sync::{mpsc, Arc};
use std::time::Duration;

#[derive(Debug, PartialEq)]
enum Event {
    Connected(bool),
    Frame(Vec<u8>),
}

#[tokio::test]
async fn stale_future_and_unknown_commands_never_reach_runtime_bus() {
    let fixture = tempfile::tempdir().expect("fixture");
    let socket = fixture.path().join("commands.sock");
    let bus = chappe::Bus::default();
    let mut commands = bus.subscribe("robot/enable");
    let fanout = chappe::ipc::IpcFanout::spawn_client(socket.clone(), bus).expect("fanout");
    let mut connected = fanout.subscribe_connection();
    let listener = std::os::unix::net::UnixListener::bind(socket).expect("peer");
    let mut peer = tokio::task::spawn_blocking(move || listener.accept().expect("accept").0)
        .await
        .expect("accept task");
    assert!(
        tokio::time::timeout(Duration::from_secs(5), connected.recv())
            .await
            .expect("connect deadline")
            .expect("connected")
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    for (topic, timestamp, value) in [
        ("robot/enable", 0, 1),
        ("robot/enable", now + 60000, 2),
        ("unknown/topic", now, 3),
        ("robot/enable", now, 4),
    ] {
        let envelope = armee_proto::Envelope {
            timestamp_ms: timestamp,
            source_node: "gateway-test".into(),
            message_type: "marengo.v1.EnableCommand".into(),
            payload: vec![value],
        }
        .encode_to_vec();
        let frame = encode_frame(chappe::ipc::DIRECTION_GATEWAY_TO_RUNTIME, topic, &envelope)
            .expect("command frame");
        peer.write_all(&frame).expect("command bytes");
    }
    let accepted = tokio::time::timeout(Duration::from_secs(5), commands.recv())
        .await
        .expect("command deadline")
        .expect("bus command");
    let accepted = armee_proto::Envelope::decode(accepted.as_slice()).expect("accepted envelope");
    assert_eq!(accepted.payload, vec![4], "only current command admitted");
    assert!(
        commands.try_recv().is_err(),
        "no stale enable queued for replay"
    );
    fanout.shutdown().expect("join runtime transport");
    drop(peer);
    fixture.close().expect("remove joined fixture");
}

#[test]
fn replacement_retires_partial_old_frame_and_disconnect_closes_command_peer() {
    let fixture = tempfile::tempdir().expect("fixture");
    let socket = fixture.path().join("peer.sock");
    let (events, received) = mpsc::channel();
    let frame_events = events.clone();
    let listener = IpcListener::spawn_server_with_lifecycle(
        socket.clone(),
        Arc::new(move |topic, payload| {
            assert_eq!(topic, "robot/state");
            frame_events
                .send(Event::Frame(payload))
                .expect("frame event");
        }),
        Arc::new(move |connected| {
            events
                .send(Event::Connected(connected))
                .expect("peer event");
        }),
    )
    .expect("listener");
    let next = || {
        received
            .recv_timeout(Duration::from_secs(5))
            .expect("peer event deadline")
    };
    let mut first = std::os::unix::net::UnixStream::connect(&socket).expect("first peer");
    first
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read deadline");
    assert_eq!(next(), Event::Connected(true));
    let old_frame =
        encode_frame(DIRECTION_RUNTIME_TO_GATEWAY, "robot/state", &[1; 8]).expect("frame");
    first
        .write_all(&old_frame[..old_frame.len() - 4])
        .expect("partial retired publication");
    let mut second = std::os::unix::net::UnixStream::connect(&socket).expect("replacement peer");
    assert_eq!(next(), Event::Connected(true));
    let mut byte = [0];
    assert_eq!(first.read(&mut byte).expect("retired peer closed"), 0);
    let current =
        encode_frame(DIRECTION_RUNTIME_TO_GATEWAY, "robot/state", &[2; 8]).expect("new frame");
    second.write_all(&current).expect("current publication");
    assert_eq!(next(), Event::Frame(vec![2; 8]));
    drop(second);
    assert_eq!(next(), Event::Connected(false));
    assert!(
        listener.send_command("robot/enable", &[1]).is_err(),
        "disconnected peer cannot admit commands"
    );
    listener.shutdown().expect("join all listener readers");
    assert!(
        received.try_recv().is_err(),
        "no retired-frame or duplicate-disconnect callback"
    );
    drop(first);
    drop(listener);
    fixture.close().expect("remove fixture after task join");
}
