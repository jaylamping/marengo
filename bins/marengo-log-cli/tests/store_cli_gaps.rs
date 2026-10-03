//! CLI coverage for store subcommands the golden suite never invoked
//! (L-marengo-log-cli-11): `session finalize`, `purge`, `journal-import`,
//! `candump page`, and the `--enrich` catalog path (L-marengo-candump-06 guard:
//! without the default `robstride-enrichment` feature the CLI refuses with
//! "requires the robstride-enrichment feature").
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use marengo_store::Store;

fn bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin("marengo-log-cli")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(root: &Path, db: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .arg("--root")
        .arg(root)
        .arg("--db")
        .arg(db)
        .args(args)
        .output()
        .expect("actual CLI child completed")
}

fn successful(output: &Output) -> String {
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).expect("CLI stdout is UTF-8")
}

#[test]
fn cli_finalize_after_register_marks_the_end() {
    let fixture = tempfile::tempdir().expect("exclusive CLI fixture");
    let root = fixture.path();
    let db = root.join("sessions.sqlite");
    let registered = run(
        root,
        &db,
        &["session", "register", "--id", "20200102T000000Z"],
    );
    assert_eq!(
        successful(&registered).trim(),
        "registered session 20200102T000000Z"
    );
    let finalized = run(
        root,
        &db,
        &["session", "finalize", "--id", "20200102T000000Z"],
    );
    assert_eq!(
        successful(&finalized).trim(),
        "finalized session 20200102T000000Z"
    );
    let store = Store::open(&db, root).expect("reopen after CLI finalize");
    let row = store
        .get_session("20200102T000000Z")
        .expect("row")
        .expect("session");
    assert!(row.ended_ms.is_some(), "finalize must stamp ended_ms");
}

#[test]
fn cli_purge_on_a_fresh_store_reports_zeros() {
    let fixture = tempfile::tempdir().expect("exclusive CLI fixture");
    let root = fixture.path();
    let db = root.join("sessions.sqlite");
    let purged = run(root, &db, &["purge"]);
    let stdout = successful(&purged);
    assert!(
        stdout.starts_with("purged 0 log rows, 0 sessions older than the archive window, 0 sessions over the disk budget"),
        "fresh store purges nothing: {stdout}"
    );
    assert!(
        stdout.contains("of 5368709120 bytes"),
        "purge must report the default disk budget: {stdout}"
    );
}

/// The non-Linux `import_journal` stub returns `Ok(0)`; the journald spawn
/// itself needs systemd and is covered by the `marengo-store` parse tests.
#[cfg(not(target_os = "linux"))]
#[test]
fn cli_journal_import_stub_reports_zero() {
    let fixture = tempfile::tempdir().expect("exclusive CLI fixture");
    let root = fixture.path();
    let db = root.join("sessions.sqlite");
    let imported = run(root, &db, &["journal-import"]);
    assert_eq!(successful(&imported).trim(), "imported 0 journal lines");
}

#[test]
fn cli_candump_page_json_paginates() {
    let file = repo_root().join("crates/marengo-candump/tests/fixtures/delta.log");
    let output = Command::new(bin())
        .args([
            "candump",
            "page",
            "--file",
            file.to_str().expect("utf8 path"),
            "--timestamp",
            "delta",
            "--format",
            "json",
            "--limit",
            "1",
        ])
        .output()
        .expect("actual CLI child completed");
    let stdout = successful(&output);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("page stdout is JSON");
    assert!(
        value.get("frames").is_some() || value.get("page").is_some(),
        "page report must carry its frames: {value}"
    );
}

/// Guard for L-marengo-candump-06: the Pi deploy keeps default features, so
/// `--enrich` must resolve the catalog from a motors.yaml directory instead
/// of refusing with "requires the robstride-enrichment feature".
#[test]
fn cli_candump_enrich_resolves_the_catalog() {
    let file = repo_root().join("crates/marengo-candump/tests/fixtures/delta.log");
    let config_dir = repo_root().join("config");
    let output = Command::new(bin())
        .args([
            "candump",
            "summary",
            "--file",
            file.to_str().expect("utf8 path"),
            "--timestamp",
            "delta",
            "--enrich",
            "--config-dir",
            config_dir.to_str().expect("utf8 path"),
        ])
        .output()
        .expect("actual CLI child completed");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("requires the robstride-enrichment feature"),
        "enrichment feature dropped from the build: {stderr}"
    );
    assert!(output.status.success(), "enriched summary failed: {stderr}");
}
