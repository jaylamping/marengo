//! # marengo-candump — candump inspection deep module
//!
//! Owns candump syntax, timestamp normalization, counting, deterministic
//! summaries, gzip detection, parsed-frame paging, and optional Robstride
//! enrichment. Accepts can-utils log (`ID#HEX`) and default ASCII
//! (`ID [dlc] XX…`) lines. Callers choose [`TimestampMode`]; there is no Auto.
//!
//! ## Responsibilities
//!
//! - [`Candump::inspect_path`] / [`Candump::inspect_bytes`]: single inspection seam
//! - [`Candump::visit_path`] / [`Candump::visit_bytes`]: stream every parsed frame to a
//!   caller visitor (whole-capture analyses such as `marengo-log-cli firmware-timing`)
//! - Validated [`CanId`], [`Summary`], [`Inspection`]
//! - Optional `robstride-enrichment` catalog lookup
//!
//! Untrusted input limits: at most 4096 bytes per physical line (including newline)
//! and 256 MiB of decompressed capture bytes. Malformed frames are skipped; finite
//! timestamps outside the representable domain fail inspection with a typed error.
//! Delta timestamps and JSON offsets must fit Duration; absolute timestamps must
//! round to fewer than 2^64 Unix microseconds. Accepted offsets retain nanosecond
//! rounding, including positive subnanosecond values rounding to zero.
//!
//! ## Does not
//!
//! - Own session/blob path lookup (marengo-store)
//! - Depend on SQLite or gateway HTTP
//! - Expose a public line parser (avoids forked caller logic)
//!
//! ## Callers
//!
//! | Caller | Usage |
//! |--------|--------|
//! | marengo-store | Delta archives / hot harness via `Candump::plain` |
//! | marengo-log-cli | Operator summary/page with required `--timestamp` |
//! | marengo-gateway | Inject enriched `Candump` when motors.yaml is valid |

mod scan;

#[cfg(feature = "robstride-enrichment")]
mod robstride;

use std::num::{NonZeroU32, NonZeroU64};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use scan::Error;

#[cfg(feature = "robstride-enrichment")]
pub use robstride::MotorCatalog;

/// Input timestamp syntax. There is intentionally no Auto variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampMode {
    /// Zero-based capture timestamps emitted by `candump -t z`.
    Delta,
    /// Unix timestamps emitted by `candump -t a`.
    Absolute,
}

/// Valid CAN 2.0 identifier: 0..=0x1FFF_FFFF, plus the kernel error-frame
/// range 0x20000000..=0x2000FFFF (`CAN_ERR_FLAG` + class).
///
/// The standard/extended distinction is wire metadata, not a value range:
/// can-utils prints extended IDs zero-padded to 8 digits
/// (`000001FE`), standard IDs unpadded (`1FE`). Inferring extended from
/// `value > 0x7FF` mislabels extended IDs at or below 0x7FF (e.g. type-0
/// replies for device <= 7), so the flag travels with the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CanId {
    value: u32,
    extended: bool,
}

impl CanId {
    pub const MAX: u32 = 0x1FFF_FFFF;
    pub const STANDARD_MAX: u32 = 0x7FF;
    /// `CAN_ERR_FLAG` (bit 29) as printed by can-utils for kernel error
    /// frames (e.g. `20000004#` on RX overflow).
    pub const ERROR_FLAG: u32 = 0x2000_0000;
    pub const ERROR_MAX: u32 = 0x2000_FFFF;

    pub fn new(value: u32, extended: bool) -> Result<Self, Error> {
        let valid = value <= Self::MAX || (Self::ERROR_FLAG..=Self::ERROR_MAX).contains(&value);
        if !valid {
            return Err(Error::InvalidCanId { value });
        }
        Ok(Self { value, extended })
    }

    pub fn get(self) -> u32 {
        self.value
    }

    pub fn is_extended(self) -> bool {
        self.extended
    }

    /// Kernel error frame (`CAN_ERR_FLAG` set): bus-state evidence, never
    /// a Robstride frame. Excluded from enrichment.
    pub fn is_error(self) -> bool {
        self.value & Self::ERROR_FLAG != 0
    }

    pub fn to_canonical_hex(self) -> String {
        if self.extended {
            format!("{:08X}", self.value)
        } else {
            format!("{:03X}", self.value)
        }
    }
}

impl Serialize for CanId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_canonical_hex())
    }
}

impl<'de> Deserialize<'de> for CanId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let text = s.trim();
        let value = u32::from_str_radix(text, 16).map_err(serde::de::Error::custom)?;
        // can-utils zero-pads extended IDs to 8 digits; standard IDs are
        // unpadded. The width is the wire flag.
        let extended = text.len() > 3;
        CanId::new(value, extended).map_err(serde::de::Error::custom)
    }
}

/// Unix time retained only for absolute input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnixMicros(u64);

impl UnixMicros {
    pub(crate) fn new(micros: u64) -> Self {
        Self(micros)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

fn serialize_offset_secs<S>(offset: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_f64(offset.as_secs_f64())
}

fn deserialize_offset_secs<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let secs = f64::deserialize(deserializer)?;
    if !secs.is_finite() || secs < 0.0 {
        return Err(serde::de::Error::custom(
            "offset must be finite and non-negative",
        ));
    }
    Duration::try_from_secs_f64(secs).map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    /// Always normalized to the first parsed frame in this inspection.
    #[serde(
        serialize_with = "serialize_offset_secs",
        deserialize_with = "deserialize_offset_secs"
    )]
    pub offset: Duration,
    /// Present only when TimestampMode::Absolute was requested.
    pub unix_time: Option<UnixMicros>,
    pub interface: String,
    pub can_id: CanId,
    /// Decoded bytes, never unchecked hexadecimal text. Empty for RTR.
    pub data: Vec<u8>,
    /// Remote transmission request: no data by definition, never enriched.
    #[serde(default)]
    pub rtr: bool,
    /// One-based physical source line, including blank/malformed predecessors.
    pub source_line: NonZeroU64,
    pub enrichment: Option<FrameEnrichment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameEnrichment {
    /// Always retained when a 29-bit Robstride extended ID can be unpacked,
    /// including communication types unknown to the current enum.
    pub comm_type: u8,
    pub comm_type_name: Option<String>,
    /// Resolved with (interface, direction-aware device_id) against motors.yaml.
    pub device_id: Option<u8>,
    pub joint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterfaceSummary {
    pub name: String,
    pub parsed_frames: u64,
    /// Count divided by whole-capture duration; None for zero duration.
    pub approx_hz: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanIdCount {
    pub can_id: CanId,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// Every logical input line, including blank/malformed and unterminated last.
    pub total_lines: u64,
    /// Lines accepted as frames, including kernel error frames and RTR.
    pub parsed_frames: u64,
    /// Physical source size: compressed path size for .gz, otherwise input bytes.
    pub source_bytes: u64,
    /// Last normalized offset; zero for fewer than two distinct timestamps.
    pub duration_s: f64,
    /// parsed_frames / duration_s, or None when duration_s == 0.
    pub approx_hz: Option<f64>,
    /// Lexically sorted by name.
    pub interfaces: Vec<InterfaceSummary>,
    /// Count descending, then numeric CAN ID ascending; at most requested limit.
    pub top_ids: Vec<CanIdCount>,
    /// Whether at least one parsed frame resolved a joint name against the
    /// catalog. False covers enrichment off (motors.yaml unloadable) and a
    /// catalog that matches nothing: consumers must not render a joint
    /// column on a bare mode flag.
    #[serde(default)]
    pub enriched: bool,
}

/// Parsed-frame paging, not source-line paging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramePage {
    offset: u64,
    limit: NonZeroU32,
}

impl FramePage {
    pub const MAX_LIMIT: u32 = 5_000;

    pub fn new(offset: u64, limit: u32) -> Result<Self, Error> {
        let limit = NonZeroU32::new(limit).ok_or(Error::InvalidPageLimit {
            actual: limit,
            max: Self::MAX_LIMIT,
        })?;
        if limit.get() > Self::MAX_LIMIT {
            return Err(Error::InvalidPageLimit {
                actual: limit.get(),
                max: Self::MAX_LIMIT,
            });
        }
        Ok(Self { offset, limit })
    }

    pub fn offset(self) -> u64 {
        self.offset
    }

    pub fn limit(self) -> u32 {
        self.limit.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InspectRequest {
    timestamp_mode: TimestampMode,
    page: Option<FramePage>,
    top_id_limit: u8,
}

impl InspectRequest {
    pub const DEFAULT_TOP_ID_LIMIT: u8 = 10;
    pub const MAX_TOP_ID_LIMIT: u8 = 64;

    pub fn summary(timestamp_mode: TimestampMode) -> Self {
        Self {
            timestamp_mode,
            page: None,
            top_id_limit: Self::DEFAULT_TOP_ID_LIMIT,
        }
    }

    pub fn page(timestamp_mode: TimestampMode, page: FramePage) -> Self {
        Self {
            timestamp_mode,
            page: Some(page),
            top_id_limit: Self::DEFAULT_TOP_ID_LIMIT,
        }
    }

    pub fn with_top_id_limit(mut self, limit: u8) -> Result<Self, Error> {
        if limit > Self::MAX_TOP_ID_LIMIT {
            return Err(Error::InvalidTopIdLimit {
                max: Self::MAX_TOP_ID_LIMIT,
            });
        }
        self.top_id_limit = limit;
        Ok(self)
    }

    pub fn timestamp_mode(self) -> TimestampMode {
        self.timestamp_mode
    }

    pub fn frame_page(self) -> Option<FramePage> {
        self.page
    }

    pub fn top_id_limit(self) -> u8 {
        self.top_id_limit
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inspection {
    pub timestamp_mode: TimestampMode,
    pub summary: Summary,
    /// Empty for summary requests; otherwise only the requested parsed-frame page.
    pub frames: Vec<Frame>,
}

/// Candump inspection facade. Enrichment is a private mode, not a public trait.
#[derive(Default)]
pub struct Candump {
    enrichment: scan::EnrichmentMode,
}

impl Candump {
    pub fn plain() -> Self {
        Self {
            enrichment: scan::EnrichmentMode::None,
        }
    }

    /// Whether Robstride enrichment is active: IDs resolve against
    /// motors.yaml. A plain parser leaves every frame unenriched.
    pub fn is_enriched(&self) -> bool {
        !matches!(self.enrichment, scan::EnrichmentMode::None)
    }

    /// Streams plain or gzip input (detected by magic bytes), retaining only
    /// summary accumulators and the requested page.
    pub fn inspect_path(
        &self,
        path: impl AsRef<Path>,
        request: InspectRequest,
    ) -> Result<Inspection, Error> {
        let path = path.as_ref();
        scan::inspect_path(path, request, &self.enrichment, None).map_err(|err| match err {
            Error::Io { source, .. } => Error::Io {
                path: path.to_path_buf(),
                source,
            },
            other => other,
        })
    }

    /// Fixture/test entry point with the same parser and summarizer.
    pub fn inspect_bytes(
        &self,
        bytes: &[u8],
        request: InspectRequest,
    ) -> Result<Inspection, Error> {
        scan::inspect_bytes(bytes, request, &self.enrichment, None)
    }

    /// Streams every parsed frame, in source order, to `visit` without retaining
    /// frames (for whole-capture analyses that cannot page); returns the summary.
    pub fn visit_path(
        &self,
        path: impl AsRef<Path>,
        timestamp_mode: TimestampMode,
        visit: &mut dyn FnMut(&Frame),
    ) -> Result<Summary, Error> {
        let path = path.as_ref();
        scan::inspect_path(
            path,
            InspectRequest::summary(timestamp_mode),
            &self.enrichment,
            Some(visit),
        )
        .map(|inspection| inspection.summary)
        .map_err(|err| match err {
            Error::Io { source, .. } => Error::Io {
                path: path.to_path_buf(),
                source,
            },
            other => other,
        })
    }

    /// [`Self::visit_path`] over in-memory bytes (fixtures and tests).
    pub fn visit_bytes(
        &self,
        bytes: &[u8],
        timestamp_mode: TimestampMode,
        visit: &mut dyn FnMut(&Frame),
    ) -> Result<Summary, Error> {
        scan::inspect_bytes(
            bytes,
            InspectRequest::summary(timestamp_mode),
            &self.enrichment,
            Some(visit),
        )
        .map(|inspection| inspection.summary)
    }

    #[cfg(feature = "robstride-enrichment")]
    pub fn with_robstride(catalog: MotorCatalog) -> Self {
        Self {
            enrichment: scan::EnrichmentMode::Robstride(catalog),
        }
    }

    #[cfg(feature = "robstride-enrichment")]
    pub fn with_robstride_from_config_dir(dir: impl AsRef<Path>) -> Result<Self, Error> {
        let motors = marengo_config::load_motors_config_from(dir)
            .map_err(|e| Error::InvalidMotorCatalog(e.to_string()))?;
        let catalog = MotorCatalog::try_from(&motors)?;
        Ok(Self::with_robstride(catalog))
    }
}

/// Format an [`Inspection`] as key=value text lines for CLI `--format text`.
pub fn format_inspection_text(inspection: &Inspection) -> String {
    let mut out = String::new();
    let s = &inspection.summary;
    push_kv(
        &mut out,
        "timestamp_mode",
        format!("{:?}", inspection.timestamp_mode).to_ascii_lowercase(),
    );
    push_kv(&mut out, "total_lines", s.total_lines);
    push_kv(&mut out, "parsed_frames", s.parsed_frames);
    push_kv(&mut out, "enriched", s.enriched);
    push_kv(&mut out, "source_bytes", s.source_bytes);
    push_kv(&mut out, "duration_s", format!("{:.6}", s.duration_s));
    match s.approx_hz {
        Some(hz) => push_kv(&mut out, "approx_hz", format!("{hz:.6}")),
        None => push_kv(&mut out, "approx_hz", "null"),
    }
    for iface in &s.interfaces {
        let hz = iface
            .approx_hz
            .map(|h| format!("{h:.6}"))
            .unwrap_or_else(|| "null".to_string());
        push_kv(
            &mut out,
            "interface",
            format!(
                "{} frames={} approx_hz={}",
                iface.name, iface.parsed_frames, hz
            ),
        );
    }
    for id in &s.top_ids {
        push_kv(
            &mut out,
            "top_id",
            format!("{} count={}", id.can_id.to_canonical_hex(), id.count),
        );
    }
    for frame in &inspection.frames {
        let data_hex: String = frame.data.iter().map(|b| format!("{b:02X}")).collect();
        let mut line = format!(
            "offset={:.6} iface={} id={} data={} line={}",
            frame.offset.as_secs_f64(),
            frame.interface,
            frame.can_id.to_canonical_hex(),
            if frame.rtr { "RTR" } else { &data_hex },
            frame.source_line
        );
        if let Some(unix) = frame.unix_time {
            line.push_str(&format!(" unix_us={}", unix.get()));
        }
        if let Some(enr) = &frame.enrichment {
            line.push_str(&format!(" comm_type={}", enr.comm_type));
            if let Some(name) = &enr.comm_type_name {
                line.push_str(&format!(" comm_type_name={name}"));
            }
            if let Some(dev) = enr.device_id {
                line.push_str(&format!(" device_id={dev}"));
            }
            if let Some(joint) = &enr.joint {
                line.push_str(&format!(" joint={joint}"));
            }
        }
        out.push_str("frame ");
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn push_kv(out: &mut String, key: &str, value: impl std::fmt::Display) {
    out.push_str(key);
    out.push('=');
    out.push_str(&value.to_string());
    out.push('\n');
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod can_id_tests {
    use super::*;

    #[test]
    fn rejects_ids_outside_data_and_error_ranges() {
        assert!(CanId::new(0x2001_0000, true).is_err());
        assert!(CanId::new(0x1FFF_FFFF, false).is_ok());
    }

    #[test]
    fn kernel_error_frames_are_valid_but_never_enriched() {
        let id = CanId::new(0x2000_0004, true).unwrap();
        assert!(id.is_error());
        assert_eq!(id.to_canonical_hex(), "20000004");
    }

    #[test]
    fn extended_flag_comes_from_wire_width_not_value() {
        // Extended 0x1FE (e.g. a type-0 reply for a low device id) is an
        // 8-digit field on the wire: extended despite value <= 0x7FF.
        let ext = CanId::new(0x1FE, true).unwrap();
        assert!(ext.is_extended());
        assert_eq!(ext.to_canonical_hex(), "000001FE");
        let std = CanId::new(0x1FE, false).unwrap();
        assert!(!std.is_extended());
        assert_eq!(std.to_canonical_hex(), "1FE");
    }

    #[test]
    fn accepts_max_extended() {
        let id = CanId::new(0x1FFF_FFFF, true).unwrap();
        assert!(id.is_extended());
        assert_eq!(id.to_canonical_hex(), "1FFFFFFF");
    }

    #[test]
    fn standard_pads_three_digits() {
        let id = CanId::new(0x701, false).unwrap();
        assert!(!id.is_extended());
        assert_eq!(id.to_canonical_hex(), "701");
    }

    #[test]
    fn canonical_hex_round_trips_through_json() {
        for (value, extended) in [(0x701u32, false), (0x1FE, true), (0x0280_02FF, true)] {
            let id = CanId::new(value, extended).unwrap();
            let json = serde_json::to_string(&id).unwrap();
            let back: CanId = serde_json::from_str(&json).unwrap();
            assert_eq!(back, id);
        }
    }
}
