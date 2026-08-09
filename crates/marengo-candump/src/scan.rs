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

pub(crate) fn inspect_path(
    path: &Path,
    request: InspectRequest,
    enrichment: &EnrichmentMode,
) -> Result<Inspection, Error> {
    let meta_len = std::fs::metadata(path)
        .map(|m| m.len())
        .map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let file = File::open(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut header = [0u8; 2];
    let mut file = file;
    let n = file.read(&mut header).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    // Re-open after peeking magic so BufRead starts at byte 0.
    drop(file);
    let file = File::open(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let gzipped = n >= 2 && header == GZIP_MAGIC;
    let reader: Box<dyn BufRead + '_> = if gzipped {
        Box::new(BufReader::new(GzDecoder::new(file)))
    } else {
        Box::new(BufReader::new(file))
    };
    scan_reader(reader, meta_len, request, enrichment)
}

pub(crate) fn inspect_bytes(
    bytes: &[u8],
    request: InspectRequest,
    enrichment: &EnrichmentMode,
) -> Result<Inspection, Error> {
    let source_bytes = bytes.len() as u64;
    let gzipped = bytes.len() >= 2 && bytes[0] == GZIP_MAGIC[0] && bytes[1] == GZIP_MAGIC[1];
    let reader: Box<dyn BufRead + '_> = if gzipped {
        Box::new(BufReader::new(GzDecoder::new(Cursor::new(bytes))))
    } else {
        Box::new(BufReader::new(Cursor::new(bytes)))
    };
    scan_reader(reader, source_bytes, request, enrichment)
}

fn scan_reader(
    mut reader: Box<dyn BufRead + '_>,
    source_bytes: u64,
    request: InspectRequest,
    enrichment: &EnrichmentMode,
) -> Result<Inspection, Error> {
    let mut acc = Accumulator::new(request);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let read = reader
            .read_until(b'\n', &mut buf)
            .map_err(|source| Error::Io {
                path: PathBuf::from("<stream>"),
                source,
            })?;
        if read == 0 {
            break;
        }
        if buf.last() == Some(&b'\n') {
            buf.pop();
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
        }
        acc.ingest_line(&buf, enrichment)?;
    }
    Ok(acc.finish(request.timestamp_mode(), source_bytes))
}

struct Accumulator {
    total_lines: u64,
    parsed_frames: u64,
    first_raw_ts: Option<f64>,
    previous_raw_ts: Option<f64>,
    last_offset: Duration,
    id_counts: HashMap<u32, u64>,
    iface_counts: HashMap<String, u64>,
    page: Option<FramePage>,
    page_frames: Vec<Frame>,
    top_id_limit: u8,
    timestamp_mode: TimestampMode,
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
        }
    }

    fn ingest_line(&mut self, buf: &[u8], enrichment: &EnrichmentMode) -> Result<(), Error> {
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
                if !(micros.is_finite() && micros >= 0.0 && micros <= u64::MAX as f64) {
                    return Ok(());
                }
                Some(UnixMicros::new(micros as u64))
            }
            TimestampMode::Delta => None,
        };

        let first = match self.first_raw_ts {
            Some(v) => v,
            None => {
                self.first_raw_ts = Some(parsed.raw_ts);
                parsed.raw_ts
            }
        };
        let offset_secs = parsed.raw_ts - first;
        let offset = Duration::from_secs_f64(offset_secs);
        self.last_offset = offset;
        self.previous_raw_ts = Some(parsed.raw_ts);

        let frame_index = self.parsed_frames;
        self.parsed_frames = self.parsed_frames.saturating_add(1);
        *self.id_counts.entry(parsed.can_id.get()).or_insert(0) += 1;
        *self
            .iface_counts
            .entry(parsed.interface.clone())
            .or_insert(0) += 1;

        if let Some(page) = self.page {
            let start = page.offset();
            let end = start.saturating_add(u64::from(page.limit()));
            if frame_index >= start && frame_index < end {
                let enrichment = enrich_frame(parsed.can_id, &parsed.interface, enrichment);
                self.page_frames.push(Frame {
                    offset,
                    unix_time,
                    interface: parsed.interface,
                    can_id: parsed.can_id,
                    data: parsed.data,
                    source_line: line_no,
                    enrichment,
                });
            }
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

        let mut top: Vec<(u32, u64)> = self.id_counts.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let top_ids = top
            .into_iter()
            .take(usize::from(self.top_id_limit))
            .filter_map(|(id, count)| {
                CanId::new(id)
                    .ok()
                    .map(|can_id| CanIdCount { can_id, count })
            })
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
}

fn parse_frame_fields(buf: &[u8]) -> Option<ParsedFields> {
    let line = std::str::from_utf8(buf).ok()?.trim();
    if line.is_empty() {
        return None;
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
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
    let id_part = parts[2];
    let (id_hex, data_hex_owned) = if let Some((id, hex)) = id_part.split_once('#') {
        let mut data = hex.to_string();
        if parts.len() > 3 {
            if !data.is_empty() {
                data.push(' ');
            }
            data.push_str(&parts[3..].join(" "));
        }
        (id, data)
    } else if parts.len() == 3 {
        (id_part, String::new())
    } else if is_ascii_dlc_token(parts[3]) {
        (id_part, parts[4..].join(" "))
    } else {
        return None;
    };

    let can_value = u32::from_str_radix(id_hex, 16).ok()?;
    let can_id = CanId::new(can_value).ok()?;

    let data_hex: String = data_hex_owned
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    if data_hex.len() % 2 != 0 {
        return None;
    }
    let byte_len = data_hex.len() / 2;
    if byte_len > MAX_CLASSIC_DLC {
        return None;
    }
    let mut data = Vec::with_capacity(byte_len);
    let bytes = data_hex.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        data.push((hi << 4) | lo);
        i += 2;
    }

    Some(ParsedFields {
        raw_ts,
        interface,
        can_id,
        data,
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
    bytes[1..bytes.len() - 1]
        .iter()
        .all(|b| b.is_ascii_digit())
}

fn enrich_frame(
    can_id: CanId,
    interface: &str,
    enrichment: &EnrichmentMode,
) -> Option<FrameEnrichment> {
    match enrichment {
        EnrichmentMode::None => {
            let _ = (can_id, interface);
            None
        }
        #[cfg(feature = "robstride-enrichment")]
        EnrichmentMode::Robstride(catalog) => enrich_robstride(can_id, interface, catalog),
    }
}

#[cfg(feature = "robstride-enrichment")]
fn enrich_robstride(
    can_id: CanId,
    interface: &str,
    catalog: &MotorCatalog,
) -> Option<FrameEnrichment> {
    if !can_id.is_extended() {
        return None;
    }
    let ext = robstride::comm::unpack_ext_id(can_id.get())?;
    let known = robstride::comm::CommunicationType::from_u8(ext.comm_type);
    let comm_type_name = known.map(comm_type_label);
    let device_id = known.map(|kind| robstride::comm::inbound_motor_device_id(can_id.get(), kind));
    let joint = device_id.and_then(|id| catalog.lookup(interface, id).map(str::to_string));
    Some(FrameEnrichment {
        comm_type: ext.comm_type,
        comm_type_name,
        device_id,
        joint,
    })
}

#[cfg(feature = "robstride-enrichment")]
fn comm_type_label(kind: robstride::comm::CommunicationType) -> String {
    match kind {
        robstride::comm::CommunicationType::OperationControl => "operation_control",
        robstride::comm::CommunicationType::OperationStatus => "operation_status",
        robstride::comm::CommunicationType::Enable => "enable",
        robstride::comm::CommunicationType::Disable => "disable",
        robstride::comm::CommunicationType::SetZeroPosition => "set_zero_position",
        robstride::comm::CommunicationType::ReadParameter => "read_parameter",
        robstride::comm::CommunicationType::WriteParameter => "write_parameter",
        robstride::comm::CommunicationType::FaultReport => "fault_report",
        robstride::comm::CommunicationType::ActiveReporting => "active_reporting",
    }
    .to_string()
}
