//! Independent decoded-wire oracle for disconnected publication freshness.
#![cfg(unix)]
#![allow(clippy::expect_used)]

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn outage_does_not_replay_superseded_state() {
    let fixture = tempfile::tempdir().expect("fixture");
    let socket = fixture.path().join("ipc.sock");
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", "outage_worker", "--nocapture"])
        .env("MARENGO_TEST_OUTAGE_SOCKET", &socket)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("isolated transport worker");
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let (tx, rx) = mpsc::channel();
    let output_reader = std::thread::spawn(move || {
        let mut output = String::new();
        let result = stdout.read_to_string(&mut output);
        let _ = tx.send((result, output));
    });
    let error_reader = std::thread::spawn(move || {
        let mut output = String::new();
        let result = stderr.read_to_string(&mut output);
        (result, output)
    });
    let received = rx.recv_timeout(Duration::from_secs(15));
    if received.is_err() {
        let _ = child.kill();
    }
    let status = child.wait().expect("reap child");
    output_reader.join().expect("join stdout");
    let (stderr_result, error) = error_reader.join().expect("join stderr");
    stderr_result.expect("read stderr");
    let output = received.expect("transport worker deadline").1;
    fixture.close().expect("remove socket fixture");
    assert!(
        status.success(),
        "isolated oracle failed:\n{output}\n{error}"
    );
}

#[test]
fn outage_worker() {
    let Some(socket) = std::env::var_os("MARENGO_TEST_OUTAGE_SOCKET") else {
        return;
    };
    let fanout =
        chappe::ipc::IpcFanout::spawn_client(socket.clone().into(), chappe::Bus::default())
            .expect("fanout");
    // No listener exists until every publication has completed. This is an
    // actual disconnected transport, independent of scheduling or RSS sampling.
    for sequence in 0_u64..1000 {
        fanout.forward_runtime_to_gateway("robot/state", &sequence.to_le_bytes());
    }
    let listener = std::os::unix::net::UnixListener::bind(socket).expect("start peer");
    let (mut peer, _) = listener.accept().expect("accept runtime");
    peer.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read deadline");
    let mut header = [0_u8; 5];
    peer.read_exact(&mut header).expect("wire header");
    let topic_len = u32::from_le_bytes(header[1..].try_into().expect("topic length")) as usize;
    assert!(topic_len < 64, "bounded fixture topic");
    let mut topic = vec![0; topic_len];
    peer.read_exact(&mut topic).expect("topic");
    let mut length = [0; 4];
    peer.read_exact(&mut length).expect("payload length");
    assert_eq!(u32::from_le_bytes(length), 8);
    let mut payload = [0; 8];
    peer.read_exact(&mut payload).expect("payload");
    assert_eq!(header[0], chappe::ipc::DIRECTION_RUNTIME_TO_GATEWAY);
    assert_eq!(topic, b"robot/state");
    assert_eq!(
        u64::from_le_bytes(payload),
        999,
        "reconnect must carry latest eligible state"
    );
}
