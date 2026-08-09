//! Contract tests against the public Candump inspection seam only.

#![allow(clippy::panic)]

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use marengo_candump::{CanId, Candump, FramePage, InspectRequest, TimestampMode};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    fs::read(fixture(name)).unwrap_or_else(|e| panic!("fixture {name} readable: {e}"))
}

fn inspect_ok(bytes: &[u8], request: InspectRequest) -> marengo_candump::Inspection {
    Candump::plain()
        .inspect_bytes(bytes, request)
        .unwrap_or_else(|e| panic!("inspect: {e}"))
}

#[test]
fn delta_summary_counts_and_rate() {
    let report = inspect_ok(
        &fixture_bytes("delta.log"),
        InspectRequest::summary(TimestampMode::Delta),
    );
    let s = &report.summary;
    assert_eq!(s.parsed_frames, 4, "four accepted frames");
    assert_eq!(s.total_lines, 4, "four input lines");
    assert!(
        (s.duration_s - 0.02).abs() < 1e-9,
        "duration from first to last"
    );
    let hz = s
        .approx_hz
        .unwrap_or_else(|| panic!("nonzero duration yields rate"));
    assert!((hz - 200.0).abs() < 1e-6, "4 frames / 0.02s = 200 Hz");
    assert!(
        report.frames.is_empty(),
        "summary request keeps no page frames"
    );
}

#[test]
fn ascii_candump_matches_log_format_delta() {
    // Default `candump -t z` (no `-L`) — harness / Pi wire shape.
    let ascii = inspect_ok(
        &fixture_bytes("ascii.log"),
        InspectRequest::summary(TimestampMode::Delta),
    );
    let compact = inspect_ok(
        &fixture_bytes("delta.log"),
        InspectRequest::summary(TimestampMode::Delta),
    );
    assert_eq!(ascii.summary.parsed_frames, compact.summary.parsed_frames);
    assert_eq!(ascii.summary.total_lines, compact.summary.total_lines);
    assert!((ascii.summary.duration_s - compact.summary.duration_s).abs() < 1e-12);
    assert_eq!(
        ascii.summary.top_ids.len(),
        compact.summary.top_ids.len(),
        "same ID multiset"
    );
    for (a, c) in ascii
        .summary
        .top_ids
        .iter()
        .zip(compact.summary.top_ids.iter())
    {
        assert_eq!(a.can_id, c.can_id);
        assert_eq!(a.count, c.count);
    }

    let page = FramePage::new(3, 1).unwrap_or_else(|e| panic!("page: {e}"));
    let framed = inspect_ok(
        &fixture_bytes("ascii.log"),
        InspectRequest::page(TimestampMode::Delta, page),
    );
    assert_eq!(framed.frames.len(), 1);
    assert_eq!(framed.frames[0].can_id.get(), 0x0280_02FF);
    assert_eq!(
        framed.frames[0].data,
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]
    );
}

#[test]
fn absolute_retains_unix_and_normalizes_offset() {
    let page = FramePage::new(0, 10).unwrap_or_else(|e| panic!("page: {e}"));
    let report = inspect_ok(
        &fixture_bytes("absolute.log"),
        InspectRequest::page(TimestampMode::Absolute, page),
    );
    assert_eq!(report.summary.parsed_frames, 3);
    assert!((report.summary.duration_s - 1.0).abs() < 1e-9);
    assert_eq!(report.frames.len(), 3);
    assert_eq!(report.frames[0].offset, Duration::ZERO);
    assert_eq!(
        report.frames[0].unix_time.map(|u| u.get()),
        Some(1_710_000_000_000_000)
    );
    assert!(
        (report.frames[2].offset.as_secs_f64() - 1.0).abs() < 1e-9,
        "last frame offset is one second"
    );
}

#[test]
fn malformed_lines_count_but_do_not_parse() {
    let report = inspect_ok(
        &fixture_bytes("malformed.log"),
        InspectRequest::summary(TimestampMode::Delta),
    );
    assert_eq!(report.summary.parsed_frames, 3, "only classic valid frames");
    assert_eq!(
        report.summary.total_lines, 9,
        "blank and bad lines still counted"
    );
}

#[test]
fn mixed_interfaces_sort_lexically_with_per_iface_counts() {
    let report = inspect_ok(
        &fixture_bytes("mixed-interface.log"),
        InspectRequest::summary(TimestampMode::Delta),
    );
    let names: Vec<_> = report
        .summary
        .interfaces
        .iter()
        .map(|i| i.name.as_str())
        .collect();
    assert_eq!(names, ["can0", "can1", "vcan0"]);
    assert_eq!(report.summary.interfaces[0].parsed_frames, 2);
    assert_eq!(report.summary.interfaces[1].parsed_frames, 1);
    assert_eq!(report.summary.interfaces[2].parsed_frames, 1);
}

#[test]
fn gzip_matches_plain_delta() {
    let plain = Candump::plain()
        .inspect_path(
            fixture("delta.log"),
            InspectRequest::summary(TimestampMode::Delta),
        )
        .unwrap_or_else(|e| panic!("plain path: {e}"));
    let gz = Candump::plain()
        .inspect_path(
            fixture("delta.log.gz"),
            InspectRequest::summary(TimestampMode::Delta),
        )
        .unwrap_or_else(|e| panic!("gzip path: {e}"));
    assert_eq!(plain.summary.parsed_frames, gz.summary.parsed_frames);
    assert_eq!(plain.summary.total_lines, gz.summary.total_lines);
    assert!((plain.summary.duration_s - gz.summary.duration_s).abs() < 1e-12);
    assert!(
        gz.summary.source_bytes < plain.summary.source_bytes,
        "gzip source_bytes is compressed on-disk size"
    );
}

#[test]
fn top_ids_sort_count_desc_then_id_asc() {
    let bytes = b"\
(0.000000) can0 702#AA
(0.001000) can0 701#AA
(0.002000) can0 702#AA
(0.003000) can0 703#AA
(0.004000) can0 701#AA
(0.005000) can0 702#AA
";
    let request = InspectRequest::summary(TimestampMode::Delta)
        .with_top_id_limit(3)
        .unwrap_or_else(|e| panic!("limit: {e}"));
    let report = inspect_ok(bytes, request);
    let ids: Vec<_> = report
        .summary
        .top_ids
        .iter()
        .map(|c| (c.can_id.get(), c.count))
        .collect();
    assert_eq!(ids, [(0x702, 3), (0x701, 2), (0x703, 1)]);
}

#[test]
fn timestamp_regression_fails_inspection() {
    let err = match Candump::plain().inspect_bytes(
        &fixture_bytes("regression.log"),
        InspectRequest::summary(TimestampMode::Delta),
    ) {
        Ok(_) => panic!("regression must fail"),
        Err(e) => e,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("timestamp regressed"),
        "error names regression: {msg}"
    );
    assert!(msg.contains("line 3"), "physical line is reported: {msg}");
}

#[test]
fn page_skips_malformed_without_holes() {
    let bytes = b"\
(0.000000) can0 701#AA
bad
(0.010000) can0 702#BB
(0.020000) can0 703#CC
";
    let page = FramePage::new(1, 1).unwrap_or_else(|e| panic!("page: {e}"));
    let report = inspect_ok(bytes, InspectRequest::page(TimestampMode::Delta, page));
    assert_eq!(report.summary.parsed_frames, 3);
    assert_eq!(report.frames.len(), 1);
    assert_eq!(report.frames[0].can_id.get(), 0x702);
    assert_eq!(report.frames[0].source_line.get(), 3);
}

#[test]
fn can_id_json_roundtrip_canonical_hex() {
    let id = CanId::new(0x028002FF).unwrap_or_else(|e| panic!("id: {e}"));
    let json = serde_json::to_string(&id).unwrap_or_else(|e| panic!("ser: {e}"));
    assert_eq!(json, "\"028002FF\"");
    let back: CanId = serde_json::from_str(&json).unwrap_or_else(|e| panic!("de: {e}"));
    assert_eq!(back, id);
}

#[test]
fn frame_page_rejects_zero_and_oversize_limit() {
    assert!(FramePage::new(0, 0).is_err());
    assert!(FramePage::new(0, FramePage::MAX_LIMIT + 1).is_err());
    assert!(FramePage::new(0, FramePage::MAX_LIMIT).is_ok());
}

#[test]
fn inspect_path_io_error_includes_path() {
    let missing = fixture("does-not-exist.log");
    let err = match Candump::plain()
        .inspect_path(&missing, InspectRequest::summary(TimestampMode::Delta))
    {
        Ok(_) => panic!("missing file"),
        Err(e) => e,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("does-not-exist.log"),
        "I/O error retains path: {msg}"
    );
}
