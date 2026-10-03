//! Smoke coverage for the thin Set Limits sync binary (L-marengo-limit-sync-06):
//! help text renders and one-sided soft flags are refused.
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use std::process::Command;

#[test]
fn help_lists_the_soft_flags() {
    let output = Command::new(assert_cmd::cargo::cargo_bin("marengo-limit-sync"))
        .arg("--help")
        .output()
        .expect("help runs");
    assert!(output.status.success(), "help must exit 0");
    let stdout = String::from_utf8(output.stdout).expect("help is UTF-8");
    assert!(
        stdout.contains("--soft-lower"),
        "help documents --soft-lower"
    );
    assert!(
        stdout.contains("--soft-upper"),
        "help documents --soft-upper"
    );
}

#[test]
fn one_sided_soft_flags_are_refused() {
    let output = Command::new(assert_cmd::cargo::cargo_bin("marengo-limit-sync"))
        .args([
            "--repo-root",
            "/tmp",
            "--joint",
            "right_elbow_pitch",
            "--lower",
            "-0.5",
            "--upper",
            "1.15",
            "--soft-lower",
            "-0.4",
        ])
        .output()
        .expect("one-sided run completes");
    assert!(
        !output.status.success(),
        "one-sided --soft-lower without --soft-upper must fail"
    );
}
