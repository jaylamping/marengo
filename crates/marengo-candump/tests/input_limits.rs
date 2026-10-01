//! Candidate boundary qualification of bounded physical lines and gzip expansion.
#![allow(clippy::expect_used)]
use flate2::{write::GzEncoder, Compression};
use marengo_candump::{Candump, Error, InspectRequest, TimestampMode};
use std::io::Write;

#[test]
fn physical_line_boundary_includes_newline_for_plain_and_gzip() {
    let valid = format!("(0) can0 701#AA{}\n", " ".repeat(4096 - 16));
    assert_eq!(valid.len(), 4096);
    let invalid = format!("{valid} ").replace("\n ", " \n");
    for bytes in [valid.as_bytes(), invalid.as_bytes()] {
        let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
        gzip.write_all(bytes).expect("gzip");
        let gzip = gzip.finish().expect("gzip footer");
        for source in [bytes, gzip.as_slice()] {
            let result = Candump::plain()
                .inspect_bytes(source, InspectRequest::summary(TimestampMode::Delta));
            if bytes.len() == 4096 {
                assert_eq!(result.expect("exact line limit").summary.parsed_frames, 1);
            } else {
                assert!(matches!(result, Err(Error::LineTooLong { max: 4096 })));
            }
        }
    }
}

fn expanded_capture(extra: bool) -> Vec<u8> {
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    let mut line = vec![b' '; 4096];
    line[4095] = b'\n';
    for _ in 0..65536 {
        gzip.write_all(&line).expect("bounded-block gzip");
    }
    if extra {
        gzip.write_all(b"\n").expect("one excess byte");
    }
    gzip.finish().expect("gzip footer")
}

#[test]
fn gzip_expansion_refuses_after_exact_capture_byte_budget() {
    let request = InspectRequest::summary(TimestampMode::Delta);
    let exact = Candump::plain()
        .inspect_bytes(&expanded_capture(false), request)
        .expect("exact capture budget");
    assert_eq!(exact.summary.total_lines, 65536);
    assert_eq!(exact.summary.parsed_frames, 0);
    let excess = Candump::plain().inspect_bytes(&expanded_capture(true), request);
    assert!(matches!(
        excess,
        Err(Error::CaptureTooLarge { max: 268435456 })
    ));
}

#[test]
fn truncated_gzip_is_an_io_error() {
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    gzip.write_all(b"(0) can0 701#AA\n").expect("gzip");
    let mut gzip = gzip.finish().expect("gzip footer");
    gzip.truncate(gzip.len() - 6);
    assert!(matches!(
        Candump::plain().inspect_bytes(&gzip, InspectRequest::summary(TimestampMode::Delta)),
        Err(Error::Io { .. })
    ));
}

#[test]
fn absolute_microsecond_upper_bound_does_not_saturate() {
    let request = InspectRequest::summary(TimestampMode::Absolute);
    // Decimal seconds whose rounded microseconds reach the excluded 2^64 boundary.
    let report = Candump::plain().inspect_bytes(b"(18446744073709.551616) can0 701#AA\n", request);
    assert!(matches!(report, Err(Error::TimestampOutOfRange { .. })));
    let before = Candump::plain().inspect_bytes(b"(18446744073709.547) can0 701#AA\n", request);
    assert_eq!(
        before
            .expect("below microsecond boundary")
            .summary
            .parsed_frames,
        1
    );
}
