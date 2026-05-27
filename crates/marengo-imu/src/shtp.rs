//! SHTP / SH-2 constants and packet helpers (ported from Adafruit CircuitPython BNO08x).

pub const DATA_BUFFER_SIZE: usize = 512;

#[allow(dead_code)]
pub const CHANNEL_SHTP_COMMAND: u8 = 0;
pub const CHANNEL_EXE: u8 = 1;
pub const CHANNEL_CONTROL: u8 = 2;
pub const CHANNEL_INPUT_SENSOR_REPORTS: u8 = 3;

pub const SET_FEATURE_COMMAND: u8 = 0xFD;
pub const GET_FEATURE_RESPONSE: u8 = 0xFC;
pub const SHTP_REPORT_PRODUCT_ID_REQUEST: u8 = 0xF9;
pub const SHTP_REPORT_PRODUCT_ID_RESPONSE: u8 = 0xF8;
pub const COMMAND_RESPONSE: u8 = 0xF1;

pub const REPORT_ACCELEROMETER: u8 = 0x01;
pub const REPORT_GYROSCOPE: u8 = 0x02;
pub const REPORT_ROTATION_VECTOR: u8 = 0x05;

/// SH-2 timestamp rebase (channel 3 batch prefix; host timing metadata).
pub const REPORT_TIMESTAMP_REBASE: u8 = 0xFA;
/// SH-2 base timestamp reference (channel 3 batch prefix; host timing metadata).
pub const REPORT_BASE_TIMESTAMP: u8 = 0xFB;

const Q_POINT_14_SCALAR: f64 = 1.0 / 16384.0;
const Q_POINT_9_SCALAR: f64 = 1.0 / 512.0;
const Q_POINT_8_SCALAR: f64 = 1.0 / 256.0;

const HEADER_LEN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketHeader {
    pub packet_byte_count: u16,
    pub channel: u8,
    pub sequence: u8,
    pub data_length: usize,
}

impl PacketHeader {
    pub fn parse(header: &[u8; HEADER_LEN]) -> Option<Self> {
        let raw_count = u16::from_le_bytes([header[0], header[1]]);
        let packet_byte_count = raw_count & !0x8000;
        if packet_byte_count == 0 || packet_byte_count == 0x7FFF {
            return None;
        }
        let channel = header[2];
        let sequence = header[3];
        let data_length = packet_byte_count.saturating_sub(HEADER_LEN as u16) as usize;
        Some(Self {
            packet_byte_count,
            channel,
            sequence,
            data_length,
        })
    }
}

pub fn build_outgoing_packet(channel: u8, sequence: u8, payload: &[u8], out: &mut [u8]) -> usize {
    let total = payload.len() + HEADER_LEN;
    out[0] = (total as u16 & 0xFF) as u8;
    out[1] = ((total as u16) >> 8) as u8;
    out[2] = channel;
    out[3] = sequence;
    out[4..4 + payload.len()].copy_from_slice(payload);
    total
}

pub fn build_set_feature_report(feature_id: u8, report_interval_us: u32) -> [u8; 17] {
    let mut report = [0u8; 17];
    report[0] = SET_FEATURE_COMMAND;
    report[1] = feature_id;
    report[5..9].copy_from_slice(&report_interval_us.to_le_bytes());
    report
}

pub fn build_product_id_request() -> [u8; 2] {
    [SHTP_REPORT_PRODUCT_ID_REQUEST, 0]
}

pub fn report_payload_length(report_id: u8) -> Option<usize> {
    match report_id {
        REPORT_ACCELEROMETER | REPORT_GYROSCOPE => Some(10),
        REPORT_ROTATION_VECTOR => Some(14),
        REPORT_TIMESTAMP_REBASE | REPORT_BASE_TIMESTAMP => Some(5),
        0x14 | 0x15 | 0x16 => Some(16), // raw accel / gyro / mag
        GET_FEATURE_RESPONSE => Some(17),
        SHTP_REPORT_PRODUCT_ID_RESPONSE | COMMAND_RESPONSE => Some(16),
        _ => None,
    }
}

/// Meta/control report IDs (0xF0+) that may prefix sensor batches on channel 3.
pub fn is_meta_report(report_id: u8) -> bool {
    matches!(report_id, REPORT_TIMESTAMP_REBASE | REPORT_BASE_TIMESTAMP)
}

pub fn split_batch_reports(data: &[u8]) -> Result<Vec<&[u8]>, String> {
    let mut slices = Vec::new();
    let mut index = 0;
    while index < data.len() {
        let report_id = data[index];
        let Some(required) = report_payload_length(report_id) else {
            // Stop at unknown IDs instead of failing the whole packet (hub may
            // emit reports we have not enabled yet).
            break;
        };
        if index + required > data.len() {
            return Err(format!(
                "incomplete report {report_id:#04x}: need {required} bytes, have {}",
                data.len() - index
            ));
        }
        slices.push(&data[index..index + required]);
        index += required;
    }
    Ok(slices)
}

pub fn parse_rotation_vector(report: &[u8]) -> Option<(f64, f64, f64, f64, u8)> {
    if report.first().copied()? != REPORT_ROTATION_VECTOR || report.len() < 14 {
        return None;
    }
    let accuracy = report[2];
    let i = i16::from_le_bytes([report[4], report[5]]) as f64 * Q_POINT_14_SCALAR;
    let j = i16::from_le_bytes([report[6], report[7]]) as f64 * Q_POINT_14_SCALAR;
    let k = i16::from_le_bytes([report[8], report[9]]) as f64 * Q_POINT_14_SCALAR;
    let real = i16::from_le_bytes([report[10], report[11]]) as f64 * Q_POINT_14_SCALAR;
    Some((i, j, k, real, accuracy))
}

pub fn parse_vec3_report(report: &[u8], expected_id: u8, scalar: f64) -> Option<[f64; 3]> {
    if report.first().copied()? != expected_id || report.len() < 10 {
        return None;
    }
    let x = i16::from_le_bytes([report[4], report[5]]) as f64 * scalar;
    let y = i16::from_le_bytes([report[6], report[7]]) as f64 * scalar;
    let z = i16::from_le_bytes([report[8], report[9]]) as f64 * scalar;
    Some([x, y, z])
}

pub fn parse_accel(report: &[u8]) -> Option<[f64; 3]> {
    parse_vec3_report(report, REPORT_ACCELEROMETER, Q_POINT_8_SCALAR)
}

pub fn parse_gyro(report: &[u8]) -> Option<[f64; 3]> {
    parse_vec3_report(report, REPORT_GYROSCOPE, Q_POINT_9_SCALAR)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn header_parse_valid() {
        let header = [0x10, 0x00, 2, 7];
        let parsed = PacketHeader::parse(&header).expect("header");
        assert_eq!(parsed.packet_byte_count, 16);
        assert_eq!(parsed.channel, 2);
        assert_eq!(parsed.sequence, 7);
        assert_eq!(parsed.data_length, 12);
    }

    #[test]
    fn header_parse_empty() {
        assert!(PacketHeader::parse(&[0, 0, 0, 0]).is_none());
    }

    #[test]
    fn build_outgoing_roundtrip() {
        let payload = [0xFD, 0x05];
        let mut buf = [0u8; 16];
        let len = build_outgoing_packet(2, 3, &payload, &mut buf);
        assert_eq!(len, 6);
        assert_eq!(&buf[4..6], &payload);
        let header = PacketHeader::parse(&buf[..4].try_into().expect("hdr")).expect("parsed");
        assert_eq!(header.channel, 2);
        assert_eq!(header.sequence, 3);
        assert_eq!(header.data_length, 2);
    }

    #[test]
    fn parse_rotation_vector_sample() {
        let mut report = [0u8; 14];
        report[0] = REPORT_ROTATION_VECTOR;
        report[2] = 3;
        let qi = (0.25 / Q_POINT_14_SCALAR) as i16;
        report[4..6].copy_from_slice(&qi.to_le_bytes());
        let (i, _j, _k, _r, accuracy) = parse_rotation_vector(&report).expect("quat");
        assert!((i - 0.25).abs() < 1e-6);
        assert_eq!(accuracy, 3);
    }

    #[test]
    fn split_batch_two_reports() {
        let mut data = vec![0u8; 24];
        data[0] = REPORT_ROTATION_VECTOR;
        data[14] = REPORT_GYROSCOPE;
        let slices = split_batch_reports(&data).expect("split");
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].len(), 14);
        assert_eq!(slices[1].len(), 10);
    }

    #[test]
    fn split_batch_with_base_timestamp_prefix() {
        let mut data = vec![0u8; 19];
        data[0] = REPORT_BASE_TIMESTAMP;
        data[5] = REPORT_ROTATION_VECTOR;
        let slices = split_batch_reports(&data).expect("split");
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].len(), 5);
        assert_eq!(slices[0][0], REPORT_BASE_TIMESTAMP);
        assert_eq!(slices[1].len(), 14);
        assert_eq!(slices[1][0], REPORT_ROTATION_VECTOR);
    }

    #[test]
    fn base_timestamp_is_meta_report() {
        assert!(is_meta_report(REPORT_BASE_TIMESTAMP));
        assert!(is_meta_report(REPORT_TIMESTAMP_REBASE));
        assert!(!is_meta_report(REPORT_ROTATION_VECTOR));
    }
}
