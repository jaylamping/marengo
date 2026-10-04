use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read};
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::time::Duration;

use flate2::read::GzDecoder;
use thiserror::Error;

use crate::{
    CanId, CanIdCount, Frame, FrameEnrichment, FramePage, InspectRequest, Inspection,
    InterfaceSummary, Summary, TimestampMode, UnixMicros,
};

#[cfg(feature = "robstride-enrichment")]
use crate::MotorCatalog;

const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];
const MAX_CLASSIC_DLC: usize = 8;
const MAX_LINE_BYTES: u64 = 4096;
const MAX_CAPTURE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum Error {
    #[error("candump I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("timestamp regressed at source line {line}: {current} < {previous}")]
    TimestampRegression {
        line: NonZeroU64,
        previous: f64,
        current: f64,
    },
    #[error("timestamp outside the representable domain at source line {line}: {value}")]
    TimestampOutOfRange { line: NonZeroU64, value: f64 },
    #[error("candump source line exceeds {max} bytes")]
    LineTooLong { max: u64 },
    #[error("candump decompressed capture exceeds {max} bytes")]
    CaptureTooLarge { max: u64 },
    #[error("page limit must be 1..={max}, got {actual}")]
    InvalidPageLimit { actual: u32, max: u32 },
    #[error("top-ID limit exceeds {max}")]
    InvalidTopIdLimit { max: u8 },
    #[error("CAN id {value:#x} exceeds 29-bit range")]
    InvalidCanId { value: u32 },
    #[cfg(feature = "robstride-enrichment")]
    #[error("invalid motor catalog: {0}")]
    InvalidMotorCatalog(String),
}

#[derive(Default)]
pub(crate) enum EnrichmentMode {
    #[default]
    None,
    #[cfg(feature = "robstride-enrichment")]
    Robstride(MotorCatalog),
}

/// Called with every parsed frame, in source order.
pub(crate) type FrameVisitor<'a> = &'a mut dyn FnMut(&Frame);

pub(crate) fn inspect_path(
    path: &Path,
    request: InspectRequest,
    enrichment: &EnrichmentMode,
    visit: Option<FrameVisitor<'_>>,
) -> Result<Inspection, Error> {
    let file = File::open(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    // Sniff gzip magic through a buffered peek: one open, no TOCTOU between
    // a metadata peek and the read, and the stream starts at byte 0.
    let mut reader = BufReader::new(file);
    let gzipped = match reader.fill_buf() {
        Ok(peek) => peek.len() >= 2 && peek[0..2] == GZIP_MAGIC,
        Err(source) => {
            return Err(Error::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let meta_len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let reader: Box<dyn BufRead + '_> = if gzipped {
        Box::new(BufReader::new(GzDecoder::new(reader)))
    } else {
        Box::new(reader)
    };
    scan_reader(reader, meta_len, request, enrichment, visit)
}

pub(crate) fn inspect_bytes(
    bytes: &[u8],
    request: InspectRequest,
    enrichment: &EnrichmentMode,
    visit: Option<FrameVisitor<'_>>,
) -> Result<Inspection, Error> {
    let source_bytes = bytes.len() as u64;
    let gzipped = bytes.len() >= 2 && bytes[0] == GZIP_MAGIC[0] && bytes[1] == GZIP_MAGIC[1];
    let reader: Box<dyn BufRead + '_> = if gzipped {
        Box::new(BufReader::new(GzDecoder::new(Cursor::new(bytes))))
    } else {
        Box::new(BufReader::new(Cursor::new(bytes)))
    };
    scan_reader(reader, source_bytes, request, enrichment, visit)
}

fn scan_reader(
    mut reader: Box<dyn BufRead + '_>,
    source_bytes: u64,
    request: InspectRequest,
    enrichment: &EnrichmentMode,
    mut visit: Option<FrameVisitor<'_>>,
) -> Result<Inspection, Error> {
    let mut acc = Accumulator::new(request);
    let mut buf = Vec::new();
    let mut consumed = 0u64;
    loop {
        buf.clear();
        // Take bounds allocation before read_until can grow the line buffer.
        let remaining = MAX_CAPTURE_BYTES - consumed;
        let read = reader
            .by_ref()
            .take((MAX_LINE_BYTES + 1).min(remaining + 1))
            .read_until(b'\n', &mut buf)
            .map_err(|source| Error::Io {
                path: PathBuf::from("<stream>"),
                source,
            })?;
        if read == 0 {
            break;
        }
        consumed += read as u64;
        if consumed > MAX_CAPTURE_BYTES {
            return Err(Error::CaptureTooLarge {
                max: MAX_CAPTURE_BYTES,
            });
        }
        if read as u64 > MAX_LINE_BYTES {
            return Err(Error::LineTooLong {
                max: MAX_LINE_BYTES,
            });
        }
        if buf.last() == Some(&b'\n') {
            buf.pop();
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
        }
        acc.ingest_line(&buf, enrichment, visit.as_deref_mut())?;
    }
    Ok(acc.finish(request.timestamp_mode(), source_bytes))
}

struct Accumulator {
    total_lines: u64,
    parsed_frames: u64,
    first_raw_ts: Option<f64>,
    previous_raw_ts: Option<f64>,
    last_offset: Duration,
    id_counts: HashMap<CanId, u64>,
    iface_counts: HashMap<String, u64>,
    page: Option<FramePage>,
    page_frames: Vec<Frame>,
    top_id_limit: u8,
    timestamp_mode: TimestampMode,
    enriched: bool,
}

impl Accumulator {
    fn new(request: InspectRequest) -> Self {
        Self {
            total_lines: 0,
            parsed_frames: 0,
            first_raw_ts: None,
            previous_raw_ts: None,
            last_offset: Duration::ZERO,
            id_counts: HashMap::new(),
            iface_counts: HashMap::new(),
            page: request.frame_page(),
            page_frames: Vec::new(),
            top_id_limit: request.top_id_limit(),
            timestamp_mode: request.timestamp_mode(),
            enriched: false,
        }
    }

    fn ingest_line<'v>(
        &mut self,
        buf: &[u8],
        enrichment: &EnrichmentMode,
        visit: Option<&mut (dyn FnMut(&Frame) + 'v)>,
    ) -> Result<(), Error> {
        self.total_lines = self.total_lines.saturating_add(1);
        let line_no = NonZeroU64::new(self.total_lines).ok_or_else(|| Error::Io {
            path: PathBuf::from("<stream>"),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, "line counter overflow"),
        })?;

        let Some(parsed) = parse_frame_fields(buf) else {
            return Ok(());
        };

        if let Some(prev) = self.previous_raw_ts {
            if parsed.raw_ts < prev {
                return Err(Error::TimestampRegression {
                    line: line_no,
                    previous: prev,
                    current: parsed.raw_ts,
                });
            }
        }

        let unix_time = match self.timestamp_mode {
            TimestampMode::Absolute => {
                let micros = (parsed.raw_ts * 1_000_000.0).round();
                // u64::MAX rounds to 2^64 in f64; equality would saturate the cast.
                if !(micros.is_finite() && (0.0..18_446_744_073_709_551_616.0).contains(&micros)) {
                    return Err(Error::TimestampOutOfRange {
                        line: line_no,
                        value: parsed.raw_ts,
                    });
                }
                Some(UnixMicros::new(micros as u64))
            }
            TimestampMode::Delta => {
                Duration::try_from_secs_f64(parsed.raw_ts).map_err(|_| {
                    Error::TimestampOutOfRange {
                        line: line_no,
                        value: parsed.raw_ts,
                    }
                })?;
                None
            }
        };

        let first = match self.first_raw_ts {
            Some(v) => v,
            None => {
                self.first_raw_ts = Some(parsed.raw_ts);
                parsed.raw_ts
            }
        };
        let offset_secs = parsed.raw_ts - first;
        let offset =
            Duration::try_from_secs_f64(offset_secs).map_err(|_| Error::TimestampOutOfRange {
                line: line_no,
                value: offset_secs,
            })?;
        self.last_offset = offset;
        self.previous_raw_ts = Some(parsed.raw_ts);

        let frame_index = self.parsed_frames;
        self.parsed_frames = self.parsed_frames.saturating_add(1);
        *self.id_counts.entry(parsed.can_id).or_insert(0) += 1;
        *self
            .iface_counts
            .entry(parsed.interface.clone())
            .or_insert(0) += 1;

        let in_page = self.page.is_some_and(|page| {
            let start = page.offset();
            let end = start.saturating_add(u64::from(page.limit()));
            frame_index >= start && frame_index < end
        });
        // RTR frames are bus-state requests, not motor frames: the id names a
        // request, not evidence, so they are never enriched.
        let enrichment = if parsed.rtr {
            None
        } else {
            enrich_frame(parsed.can_id, &parsed.data, &parsed.interface, enrichment)
        };
        // Result-side flag: at least one parsed frame (paged or not)
        // resolved a joint name. Mode-alone would claim enrichment a catalog
        // that matches nothing cannot deliver.
        if enrichment.as_ref().is_some_and(|e| e.joint.is_some()) {
            self.enriched = true;
        }
        if !in_page && visit.is_none() {
            return Ok(());
        }
        let frame = Frame {
            offset,
            unix_time,
            interface: parsed.interface,
            can_id: parsed.can_id,
            data: parsed.data,
            rtr: parsed.rtr,
            source_line: line_no,
            enrichment,
        };
        if let Some(visit) = visit {
            visit(&frame);
        }
        if in_page {
            self.page_frames.push(frame);
        }

        Ok(())
    }

    fn finish(self, timestamp_mode: TimestampMode, source_bytes: u64) -> Inspection {
        let duration_s = self.last_offset.as_secs_f64();
        let approx_hz = if duration_s > 0.0 {
            Some(self.parsed_frames as f64 / duration_s)
        } else {
            None
        };

        let mut interfaces: Vec<InterfaceSummary> = self
            .iface_counts
            .into_iter()
            .map(|(name, parsed_frames)| InterfaceSummary {
                name,
                parsed_frames,
                approx_hz: if duration_s > 0.0 {
                    Some(parsed_frames as f64 / duration_s)
                } else {
                    None
                },
            })
            .collect();
        interfaces.sort_by(|a, b| a.name.cmp(&b.name));

        let mut top: Vec<(CanId, u64)> = self.id_counts.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let top_ids = top
            .into_iter()
            .take(usize::from(self.top_id_limit))
            .map(|(can_id, count)| CanIdCount { can_id, count })
            .collect();

        Inspection {
            timestamp_mode,
            summary: Summary {
                total_lines: self.total_lines,
                parsed_frames: self.parsed_frames,
                source_bytes,
                duration_s,
                approx_hz,
                interfaces,
                top_ids,
                enriched: self.enriched,
            },
            frames: self.page_frames,
        }
    }
}

struct ParsedFields {
    raw_ts: f64,
    interface: String,
    can_id: CanId,
    data: Vec<u8>,
    /// Remote transmission request: carries no data by definition.
    rtr: bool,
}

fn parse_frame_fields(buf: &[u8]) -> Option<ParsedFields> {
    let line = std::str::from_utf8(buf).ok()?.trim();
    if line.is_empty() {
        return None;
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
    // can-utils appends a trailing `ERRORFRAME` marker to kernel error
    // frames in ASCII mode (`20000004 [8] … ERRORFRAME`, lib.c
    // `snprintf_long_canframe`). It is rendering, not payload: only error
    // frames carry it, and it is never valid hex, so drop it before parsing.
    let parts: Vec<&str> = match parts.as_slice() {
        [rest @ .., "ERRORFRAME"] => rest.to_vec(),
        _ => parts,
    };
    if parts.len() < 3 {
        return None;
    }

    let ts_raw = parts[0].trim_start_matches('(').trim_end_matches(')');
    let raw_ts: f64 = ts_raw.parse().ok()?;
    if !raw_ts.is_finite() || raw_ts < 0.0 {
        return None;
    }

    let interface = parts[1].to_string();
    if interface.is_empty() {
        return None;
    }

    // Accept both can-utils wire shapes used on the bench:
    // - log (`-L`): `(ts) iface ID#HEX…`
    // - ASCII (default `candump -t z`): `(ts) iface ID [dlc] XX YY…`
    //
    // The ID field width is the standard/extended flag: can-utils prints
    // extended IDs zero-padded to 8 digits (`000001FE`) and standard IDs
    // unpadded (`1FE`). Inferring extended from the value mislabels
    // extended IDs at or below 0x7FF.
    let id_part = parts[2];
    // The `#` separates the ID from payload: measure the width on the ID
    // field alone (`701#...` is a standard ID with data attached).
    let id_hex_prefix = id_part.split('#').next().unwrap_or("");
    let extended = id_hex_prefix.len() > 3;
    let mut declared_dlc = None;
    let (id_hex, data_hex_owned, ascii_rtr) = if let Some((id, hex)) = id_part.split_once('#') {
        let mut data = hex.to_string();
        if parts.len() > 3 {
            if !data.is_empty() {
                data.push(' ');
            }
            data.push_str(&parts[3..].join(" "));
        }
        (id, data, false)
    } else if parts.len() == 3 {
        (id_part, String::new(), false)
    } else if is_ascii_dlc_token(parts[3]) {
        let count = parts[3][1..parts[3].len() - 1].parse::<usize>().ok()?;
        if count > MAX_CLASSIC_DLC {
            return None;
        }
        declared_dlc = Some(count);
        // ASCII RTR shape: `(ts) iface ID [dlc] remote request`.
        let tail = &parts[4..];
        if tail == ["remote", "request"] {
            (id_part, String::new(), true)
        } else {
            (id_part, tail.join(" "), false)
        }
    } else {
        return None;
    };

    let can_value = u32::from_str_radix(id_hex, 16).ok()?;
    let can_id = CanId::new(can_value, extended).ok()?;

    let data_hex: String = data_hex_owned
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    // Log-format RTR shape: `ID#R` or `ID#R<len>` (remote request, no data).
    let log_rtr = data_hex
        .strip_prefix('R')
        .is_some_and(|len| len.len() <= 1 && len.bytes().all(|b| (b'0'..=b'8').contains(&b)));
    let rtr = ascii_rtr || log_rtr;
    if !rtr && data_hex.len() % 2 != 0 {
        return None;
    }
    let byte_len = if rtr { 0 } else { data_hex.len() / 2 };
    // RTR frames name a requested length (`[8] remote request`), not a
    // payload, so the declared-DLC agreement check applies to data only.
    if byte_len > MAX_CLASSIC_DLC || (!rtr && declared_dlc.is_some_and(|count| count != byte_len)) {
        return None;
    }
    let mut data = Vec::with_capacity(byte_len);
    if !rtr {
        let bytes = data_hex.as_bytes();
        let mut i = 0;
        while i + 1 < bytes.len() {
            let hi = hex_nibble(bytes[i])?;
            let lo = hex_nibble(bytes[i + 1])?;
            data.push((hi << 4) | lo);
            i += 2;
        }
    }

    Some(ParsedFields {
        raw_ts,
        interface,
        can_id,
        data,
        rtr,
    })
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Default can-utils ASCII DLC field, e.g. `[8]` or `[0]`.
fn is_ascii_dlc_token(token: &str) -> bool {
    let bytes = token.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'[' || bytes[bytes.len() - 1] != b']' {
        return false;
    }
    bytes[1..bytes.len() - 1].iter().all(|b| b.is_ascii_digit())
}

#[cfg(feature = "robstride-enrichment")]
fn enrich_frame(
    can_id: CanId,
    data: &[u8],
    interface: &str,
    enrichment: &EnrichmentMode,
) -> Option<FrameEnrichment> {
    match enrichment {
        EnrichmentMode::None => None,
        EnrichmentMode::Robstride(catalog) => enrich_robstride(can_id, data, interface, catalog),
    }
}

#[cfg(not(feature = "robstride-enrichment"))]
fn enrich_frame(
    _can_id: CanId,
    _data: &[u8],
    _interface: &str,
    _enrichment: &EnrichmentMode,
) -> Option<FrameEnrichment> {
    None
}

#[cfg(feature = "robstride-enrichment")]
fn enrich_robstride(
    can_id: CanId,
    data: &[u8],
    interface: &str,
    catalog: &MotorCatalog,
) -> Option<FrameEnrichment> {
    if !can_id.is_extended() || can_id.is_error() {
        return None;
    }
    let ext = robstride::comm::unpack_ext_id(can_id.get())?;
    let known = robstride::comm::CommunicationType::from_u8(ext.comm_type);
    let comm_type_name = known.map(|kind| kind.name().to_string());
    let device_id = known.and_then(|kind| motor_device_id(can_id.get(), data, kind));
    let joint = device_id.and_then(|id| catalog.lookup(interface, id).map(str::to_string));
    Some(FrameEnrichment {
        comm_type: ext.comm_type,
        comm_type_name,
        device_id,
        joint,
    })
}

/// Motor id for a RobStride frame, decoded with the same direction rule as
/// the receive path (`wire::classify_frame` with [`DEFAULT_HOST_ID`]): drive
/// frames echo the host id in the low byte with the responder at bits 8-15,
/// host frames carry the target in the low byte.
///
/// Returns `None` when the id bytes name no motor (reply markers, the host
/// id on a truncated reply) so a joint is never attributed to the wrong
/// motor. The bench standardises on [`DEFAULT_HOST_ID`]; captures from a
/// custom host id attribute host-side frames only.
#[cfg(feature = "robstride-enrichment")]
fn motor_device_id(
    can_id: u32,
    payload: &[u8],
    comm_type: robstride::comm::CommunicationType,
) -> Option<u8> {
    use robstride::comm::CommunicationType as T;
    use robstride::{DEFAULT_HOST_ID, DEVICE_ID_REPLY_MARKER};
    let low = (can_id & 0xFF) as u8;
    let extra_low = ((can_id >> 8) & 0xFF) as u8;
    match comm_type {
        // Type-0 replies carry the marker in the low byte and the responder
        // in extra_data; anything else is a request naming its target.
        T::GetDeviceId => {
            if low == DEVICE_ID_REPLY_MARKER {
                robstride::decode_device_id_reply(can_id, payload).map(|reply| reply.device_id)
            } else {
                Some(low)
            }
        }
        // Type-17 replies echo the host id in the low byte; requests name
        // the target motor there.
        T::ReadParameter => {
            if low == DEFAULT_HOST_ID {
                robstride::decode_read_parameter_reply(DEFAULT_HOST_ID, can_id, payload)
                    .map(|reply| reply.device_id)
            } else {
                Some(low)
            }
        }
        // Status/fault/report frames share the inbound layout; host-side
        // commands reusing these type bytes name the target in the low byte.
        T::OperationStatus | T::FaultReport | T::ActiveReporting => {
            if low == DEFAULT_HOST_ID {
                Some(extra_low)
            } else {
                Some(low)
            }
        }
        _ => Some(low),
    }
}

#[cfg(all(test, feature = "robstride-enrichment"))]
#[allow(clippy::unwrap_used)]
mod enrichment_tests {
    use marengo_config::{MotorBenchLimits, MotorEntry, MotorType, MotorsConfigFile};

    use super::*;
    use crate::{Candump, FramePage, InspectRequest, TimestampMode};

    fn catalog() -> MotorCatalog {
        let motor = |joint: &str, device_id: u8| MotorEntry {
            joint: joint.to_string(),
            driver: "robstride".to_string(),
            motor_type: MotorType::Rs02,
            can_interface: "can0".to_string(),
            device_id,
            direction: 1,
            gear_ratio: 1.0,
            recv_can_id: 0,
            firmware_version: "test".to_string(),
            bench: MotorBenchLimits {
                position_lower_rad: -1.0,
                position_upper_rad: 1.0,
                velocity_limit_rad_s: 1.0,
                torque_limit_nm: 1.0,
            },
        };
        MotorCatalog::try_from(&MotorsConfigFile {
            motors: vec![motor("j1", 1), motor("j2", 2)],
        })
        .unwrap()
    }

    fn inspect_first(catalog: MotorCatalog, line: &str) -> Frame {
        let bytes = format!("(0.000000) can0 {line}\n");
        let page = FramePage::new(0, 10).unwrap();
        let report = Candump::with_robstride(catalog)
            .inspect_bytes(
                bytes.as_bytes(),
                InspectRequest::page(TimestampMode::Delta, page),
            )
            .unwrap();
        assert_eq!(report.summary.parsed_frames, 1);
        assert_eq!(report.frames.len(), 1);
        report.frames.into_iter().next().unwrap()
    }

    fn joint_of(frame: &Frame) -> Option<&str> {
        frame.enrichment.as_ref().and_then(|e| e.joint.as_deref())
    }

    #[test]
    fn inbound_status_attributes_responder_not_host() {
        // Bench wire shape: type 2, motor 1 at bits 8-15, host 0xFD low.
        let frame = inspect_first(catalog(), "028001FD#0011223344556677");
        assert_eq!(joint_of(&frame), Some("j1"));
    }

    #[test]
    fn outbound_reporting_command_attributes_target() {
        // Host-side type-24 On to motor 2 (SocketCAN echo): the old inbound
        // decode read bits 8-15 (0xFD = 253, no joint).
        let (id, payload) = robstride::encode_active_reporting(0xFD, 2, true);
        let line = format!("{:08X}#{}", id, hex(&payload));
        let frame = inspect_first(catalog(), &line);
        assert_eq!(id, 0x1800_FD02);
        assert_eq!(joint_of(&frame), Some("j2"));
    }

    #[test]
    fn type0_reply_resolves_responder_via_marker() {
        // Type-0 reply from motor 1: marker 0xFE low, responder in
        // extra_data, 8-byte UID. Note the 8-digit width: value 0x1FE is
        // extended on the wire despite fitting in 11 bits.
        let frame = inspect_first(catalog(), "000001FE#0001020304050607");
        assert_eq!(joint_of(&frame), Some("j1"));
    }

    #[test]
    fn type0_request_names_target_in_low_byte() {
        // Type-0 request to motor 2 (host 0xFD in extra_data): not a reply.
        let frame = inspect_first(catalog(), "0000FD02#");
        assert_eq!(joint_of(&frame), Some("j2"));
    }

    #[test]
    fn type17_reply_resolves_responder_for_host() {
        // Type-17 reply to host 0xFD: responder in extra_data. The old
        // low-byte decode read 0xFD = 253 and resolved nothing.
        let frame = inspect_first(catalog(), "110001FD#0011223344556677");
        assert_eq!(joint_of(&frame), Some("j1"));
    }

    #[test]
    fn type17_request_names_target_in_low_byte() {
        let frame = inspect_first(catalog(), "11701902#");
        let name = frame
            .enrichment
            .as_ref()
            .and_then(|e| e.comm_type_name.clone());
        assert_eq!(name.as_deref(), Some("read_parameter"));
        assert_eq!(joint_of(&frame), Some("j2"));
    }

    #[test]
    fn truncated_reply_shaped_frame_resolves_nothing() {
        // Low byte names the host, but the short payload cannot confirm a
        // reply: unknown, never motor 253's joint.
        let frame = inspect_first(catalog(), "110001FD#AABBCC");
        let enrichment = frame.enrichment.as_ref().unwrap();
        assert_eq!(enrichment.comm_type_name.as_deref(), Some("read_parameter"));
        assert_eq!(enrichment.device_id, None);
        assert_eq!(joint_of(&frame), None);
    }

    #[test]
    fn error_frame_with_catalog_stays_unenriched() {
        // 0x20000004 would unpack as comm type 0: must not read as
        // "get_device_id".
        let frame = inspect_first(catalog(), "20000004#00000000");
        assert!(frame.can_id.is_error());
        assert!(frame.enrichment.is_none());
    }

    #[test]
    fn rtr_with_catalog_stays_unenriched() {
        let frame = inspect_first(catalog(), "028001FD#R");
        assert!(frame.rtr);
        assert!(frame.enrichment.is_none());
    }

    #[test]
    fn summary_enriched_reflects_joint_resolution() {
        let page = FramePage::new(0, 10).unwrap();
        let request = || InspectRequest::page(TimestampMode::Delta, page);
        let bytes = b"(0.000000) can0 028001FD#0011223344556677\n";
        let matched = Candump::with_robstride(catalog())
            .inspect_bytes(bytes, request())
            .unwrap();
        assert!(matched.summary.enriched);
        let empty = Candump::with_robstride(
            MotorCatalog::try_from(&MotorsConfigFile { motors: vec![] }).unwrap(),
        )
        .inspect_bytes(bytes, request())
        .unwrap();
        assert!(
            !empty.summary.enriched,
            "catalog matching nothing resolves no joints"
        );
        let plain = Candump::plain().inspect_bytes(bytes, request()).unwrap();
        assert!(!plain.summary.enriched);
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02X}")).collect()
    }
}
