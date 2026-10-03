//! Shared test harness helpers for the store integration tests.
//!
//! `bounded` runs a migration test body in a child process with a deadlock
//! deadline: a hung migration fails the test instead of hanging CI.

#![allow(clippy::expect_used, clippy::panic)]

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Run `worker` in a child test process named `name`, failing if it exceeds
/// the deadlock deadline. Replaces four identical per-file copies.
pub fn bounded(name: &str, worker: fn()) {
    const ENV: &str = "MARENGO_G15_CONTRACT_WORKER";
    if std::env::var(ENV).ok().as_deref() == Some(name) {
        worker();
        return;
    }
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", name, "--nocapture"])
        .env(ENV, name)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("bounded migration test child");
    let mut stdout = child.stdout.take().expect("child output pipe");
    let (finished, completion) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut output = String::new();
        let read = stdout.read_to_string(&mut output);
        let _ = finished.send((read, output));
    });
    match completion.recv_timeout(Duration::from_secs(15)) {
        Ok((read, output)) => {
            read.expect("child pipe read");
            let status = child.wait().expect("reap completed child");
            reader.join().expect("completed output reader");
            assert!(status.success(), "bounded child failed: {output}");
        }
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            panic!("bounded child exceeded its deadlock deadline: {error}");
        }
    }
}
