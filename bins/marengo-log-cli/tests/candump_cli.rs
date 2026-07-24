use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(name: &str) -> PathBuf {
    repo_root()
        .join("crates/marengo-candump/tests/fixtures")
        .join(name)
}

fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name)
}

#[test]
fn candump_summary_json_matches_golden() {
    let bin = assert_cmd::cargo::cargo_bin("marengo-log-cli");
    let output = Command::new(bin)
        .args([
            "candump",
            "summary",
            "--file",
            fixture("delta.log").to_str().expect("utf8 path"),
            "--timestamp",
            "delta",
            "--format",
            "json",
        ])
        .output()
        .expect("run candump summary");
    assert!(
        output.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is json");
    let expected: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(golden("delta_summary.json")).expect("golden readable"),
    )
    .expect("golden is json");
    assert_eq!(actual, expected, "CLI json stdout drifted from golden");
}

#[test]
fn candump_summary_text_emits_key_values() {
    let bin = assert_cmd::cargo::cargo_bin("marengo-log-cli");
    let output = Command::new(bin)
        .args([
            "candump",
            "summary",
            "--file",
            fixture("delta.log").to_str().expect("utf8 path"),
            "--timestamp",
            "delta",
            "--format",
            "text",
        ])
        .output()
        .expect("run candump text");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("parsed_frames=4"), "text: {text}");
    assert!(text.contains("approx_hz=200"), "text: {text}");
    assert!(text.contains("timestamp_mode=delta"), "text: {text}");
}
