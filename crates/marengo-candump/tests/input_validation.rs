//! Untrusted captures and decoded frames must refuse unrepresentable inputs without unwind.
#![allow(clippy::expect_used)]
use marengo_candump::{Candump, Frame, FramePage, InspectRequest, TimestampMode};
use std::panic::catch_unwind;
use std::time::Duration;

#[test]
fn huge_finite_capture_timestamps_are_input_errors() {
    for mode in [TimestampMode::Delta, TimestampMode::Absolute] {
        for input in [
            b"(0) can0 701#AA\n(1e30) can0 701#BB\n".as_slice(),
            b"(1e30) can0 701#AA\n".as_slice(),
        ] {
            let result = catch_unwind(|| {
                Candump::plain().inspect_bytes(input, InspectRequest::summary(mode))
            });
            assert!(
                result.is_ok(),
                "untrusted timestamp must not unwind: {mode:?}"
            );
            assert!(
                result.expect("no unwind").is_err(),
                "out-of-range timestamp must refuse: {mode:?}"
            );
        }
    }
}

fn frame_json(offset: &str) -> String {
    format!(
        r#"{{"offset":{offset},"unix_time":null,"interface":"can0","can_id":"701","data":[170],"source_line":1,"enrichment":null}}"#
    )
}

#[test]
fn huge_json_offsets_are_serde_errors() {
    for offset in ["1e30", "18446744073709551616", "-1", "1e400"] {
        let input = frame_json(offset);
        let result = catch_unwind(|| serde_json::from_str::<Frame>(&input));
        assert!(result.is_ok(), "JSON offset must not unwind: {offset}");
        assert!(
            result.expect("no unwind").is_err(),
            "JSON offset must refuse: {offset}"
        );
    }
}

#[test]
fn ascii_dlc_must_match_classic_payload() {
    let bytes = b"(0) can0 701 [2] AA\n(1) can0 701 [0] AA\n(2) can0 701 [9] AA\n(3) can0 701 [1] AA\n(4) can0 702 [0]\n";
    let request = InspectRequest::page(TimestampMode::Delta, FramePage::new(0, 10).expect("page"));
    let report = Candump::plain()
        .inspect_bytes(bytes, request)
        .expect("malformed frame policy");
    assert_eq!(report.summary.total_lines, 5);
    assert_eq!(report.summary.parsed_frames, 2);
    assert_eq!(
        report
            .frames
            .iter()
            .map(|f| f.source_line.get())
            .collect::<Vec<_>>(),
        vec![4, 5]
    );
    assert_eq!(report.frames[0].data, vec![170]);
    assert!(report.frames[1].data.is_empty());
}

#[test]
fn excessive_source_lines_refuse() {
    let input = format!("(0) can0 701#AA{}\n", " ".repeat(8192));
    assert!(Candump::plain()
        .inspect_bytes(
            input.as_bytes(),
            InspectRequest::summary(TimestampMode::Delta)
        )
        .is_err());
}

#[test]
fn numeric_rounding_and_malformed_frames_keep_their_contract() {
    for (input, expected) in [
        ("0", Duration::ZERO),
        ("-0.0", Duration::ZERO),
        ("1e-300", Duration::ZERO),
        ("0.000000001", Duration::from_nanos(1)),
        (
            "18446744073709549568",
            Duration::from_secs(18446744073709549568),
        ),
    ] {
        let frame: Frame = serde_json::from_str(&frame_json(input)).expect("representable offset");
        assert_eq!(frame.offset, expected);
    }
    let request = InspectRequest::page(TimestampMode::Delta, FramePage::new(0, 10).expect("page"));
    let report=Candump::plain().inspect_bytes(b"(NaN) can0 701#AA\n(inf) can0 701#AA\n(-1) can0 701#AA\n(2) can0 701#AA\n(2.000000001) can0 701#BB\n", request).expect("malformed timestamps skip");
    assert_eq!(report.summary.total_lines, 5);
    assert_eq!(report.summary.parsed_frames, 2);
    assert_eq!(report.frames[0].offset, Duration::ZERO);
    assert_eq!(report.frames[1].offset, Duration::from_nanos(1));
}
