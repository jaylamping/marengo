//! The installed systemd unit must run marengo-pi without stdin control:
//! systemd's null stdin reads as EOF, EOF quits, and `Restart=always` would
//! then restart the runtime every few seconds (RT-02).

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

fn repo_file(relative: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    std::fs::read_to_string(&path).expect("repo file")
}

#[test]
fn unit_runs_without_stdin_control() {
    let unit = repo_file("scripts/systemd/marengo-pi.service");
    let exec_start: Vec<&str> = unit
        .lines()
        .filter(|line| line.starts_with("ExecStart="))
        .collect();
    assert_eq!(exec_start.len(), 1, "one ExecStart: {exec_start:?}");
    assert!(
        exec_start[0]
            .split_whitespace()
            .any(|arg| arg == "--no-stdin-ctl"),
        "{}",
        exec_start[0]
    );
}

#[test]
fn installer_keeps_no_stdin_ctl_when_it_rewrites_exec_start() {
    let installer = repo_file("scripts/install-pi.sh");
    let rewrites: Vec<&str> = installer
        .lines()
        .filter(|line| line.contains("ExecStart=") && line.contains("marengo-pi.service"))
        .collect();
    assert!(
        !rewrites.is_empty(),
        "installer no longer rewrites ExecStart"
    );
    for line in rewrites {
        assert!(line.contains("--no-stdin-ctl"), "{line}");
    }
}
