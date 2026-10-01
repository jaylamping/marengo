//! Actual CLI reports an input error rather than unwinding on huge capture timestamps.
#![allow(clippy::expect_used)]
use std::process::Command;
#[test]
fn huge_candump_timestamp_returns_input_error() {
    let directory = tempfile::tempdir().expect("fixture");
    let input = directory.path().join("huge.log");
    std::fs::write(&input, b"(0) can0 701#AA\n(1e30) can0 701#BB\n").expect("fixture write");
    let output = Command::new(assert_cmd::cargo::cargo_bin("marengo-log-cli"))
        .args(["candump", "summary", "--timestamp", "delta", "--file"])
        .arg(&input)
        .output()
        .expect("CLI");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let cleanup = directory.close();
    assert!(cleanup.is_ok());
    assert_eq!(
        output.status.code(),
        Some(1),
        "input error exit, stderr={stderr}"
    );
    assert!(
        stderr.contains("timestamp") && !stderr.contains("panicked"),
        "input diagnostic: {stderr}"
    );
}
