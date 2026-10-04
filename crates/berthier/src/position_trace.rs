//! High-rate CSV trace for position-hold bench debugging (`MARENGO_POSITION_TRACE`).

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::OnceLock;

/// Buffered CSV writer for position-hold diagnostics (optional, env-gated).
#[derive(Debug)]
pub struct PositionTrace {
    writer: BufWriter<File>,
    period_ticks: u64,
    byte_cap: u64,
    bytes_written: u64,
    failed: bool,
    write_errors: u64,
}

/// Session cap so an unattended trace cannot fill the disk (L-berthier-07).
/// At full 5-joint 200 Hz (~150 KB/s) this is roughly half an hour.
pub const TRACE_SESSION_CAP_BYTES: u64 = 256 * 1024 * 1024;

const TRACE_BUFFER_BYTES: usize = 512 * 1024;

impl PositionTrace {
    /// Open trace file when `MARENGO_POSITION_TRACE` is set to a writable path.
    ///
    /// Sample rate defaults to `loop_hz` unless `MARENGO_POSITION_TRACE_HZ` is set.
    pub fn from_env(loop_hz: u32) -> Option<Self> {
        let path = std::env::var_os("MARENGO_POSITION_TRACE")?;
        match Self::open(Path::new(&path), loop_hz) {
            Ok(trace) => Some(trace),
            Err(error) => {
                tracing::warn!(
                    path = ?path,
                    error = %error,
                    "position trace disabled: cannot open file"
                );
                None
            }
        }
    }

    fn open(path: &Path, loop_hz: u32) -> std::io::Result<Self> {
        Self::open_inner(path, loop_hz, TRACE_BUFFER_BYTES, TRACE_SESSION_CAP_BYTES)
    }

    fn open_inner(
        path: &Path,
        loop_hz: u32,
        buffer_bytes: usize,
        byte_cap: u64,
    ) -> std::io::Result<Self> {
        let trace_hz = std::env::var("MARENGO_POSITION_TRACE_HZ")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|&hz| hz > 0)
            .unwrap_or(loop_hz);
        let period_ticks = (u64::from(loop_hz.max(1)) / u64::from(trace_hz.max(1))).max(1);
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let existing = file.metadata().map(|m| m.len()).unwrap_or(0);
        // Never append to (or grow) an already huge file across restarts.
        if existing > byte_cap {
            return Err(std::io::Error::new(
                std::io::ErrorKind::QuotaExceeded,
                "position trace file already exceeds session cap",
            ));
        }
        let is_new = existing == 0;
        let mut writer = BufWriter::with_capacity(buffer_bytes.max(1), file);
        if is_new {
            writeln!(
                writer,
                "tick,t_ms,joint,q,dq,q_traj,dq_traj,q_des,target,target_raw,q_env_lo,q_env_hi,lead,lead_sat,settle_error,phase,friction_mode,tau_p,tau_g,tau_f,tau_d,tau_ff_cmd,tau_meas,dq_mit,kp,kd,joint_stuck,planner_frozen,retarget_age_ms,planner_event,law,q_ref,dq_ref,time_scale,tau_i,kd_mit,tau_ff_wire"
            )?;
        }
        log_trace_enabled_once(path);
        Ok(Self {
            writer,
            period_ticks,
            byte_cap,
            bytes_written: 0,
            failed: false,
            write_errors: 0,
        })
    }

    #[cfg(test)]
    pub(crate) fn open_for_test(path: &Path, loop_hz: u32) -> std::io::Result<Self> {
        Self::open(path, loop_hz)
    }

    /// Write failures observed since open (tick writes stop after the first).
    /// Test-observation seam for the counted-disable path.
    #[cfg(test)]
    pub fn write_errors(&self) -> u64 {
        self.write_errors
    }

    fn note_write_result(&mut self, result: std::io::Result<()>) {
        if result.is_err() {
            self.failed = true;
            self.write_errors = self.write_errors.saturating_add(1);
            tracing::warn!("position trace write failed; tracing disabled");
        }
    }

    /// Record one sample when `tick` matches the decimation period.
    ///
    /// Infallible by design: a failed tick write disables tracing with one
    /// warning instead of erroring (or retry-failing) every tick.
    pub fn maybe_record(&mut self, tick: u64, t_ms: u64, row: &PositionTraceRow<'_>) {
        if self.failed || tick % self.period_ticks != 0 {
            return;
        }
        let line = row.format_csv_with_meta(tick, t_ms);
        if self.bytes_written.saturating_add(line.len() as u64) > self.byte_cap {
            self.failed = true;
            tracing::warn!(
                cap_bytes = self.byte_cap,
                "position trace session cap reached; tracing disabled"
            );
            return;
        }
        let result = writeln!(self.writer, "{line}");
        self.note_write_result(result);
        if self.failed {
            return;
        }
        self.bytes_written = self.bytes_written.saturating_add(line.len() as u64);
    }

    #[allow(dead_code)]
    pub fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

/// One position-hold trace row (command vs measured).
pub struct PositionTraceRow<'a> {
    pub joint: &'a str,
    pub q: f64,
    pub dq: f64,
    pub q_traj: f64,
    pub dq_traj: f64,
    pub q_des: f64,
    pub target: f64,
    pub target_raw: f64,
    pub q_env_lo: f64,
    pub q_env_hi: f64,
    pub lead: f64,
    pub lead_sat: bool,
    pub settle_error: f64,
    pub phase: &'a str,
    pub friction_mode: &'a str,
    pub tau_p: f64,
    pub tau_g: f64,
    pub tau_f: f64,
    pub tau_d: f64,
    pub tau_ff_cmd: f64,
    pub tau_meas: f64,
    pub dq_mit: f64,
    pub kp: f64,
    pub kd: f64,
    pub joint_stuck: bool,
    pub planner_frozen: bool,
    pub retarget_age_ms: u64,
    pub planner_event: &'a str,
    /// Position law (`legacy` | `scaled_pd`, ADR 0039).
    pub law: &'a str,
    /// Reference position / velocity the law commands (before the envelope clamp).
    pub q_ref: f64,
    pub dq_ref: f64,
    /// Reference governor scale `s` (scaled PD); legacy: 0 while frozen, else 1.
    pub time_scale: f64,
    /// Integral torque term (Nm).
    pub tau_i: f64,
    /// Wire kd sent to the drive.
    pub kd_mit: f64,
    /// τ_ff Davout sent after its cap and rate limiter (Nm).
    pub tau_ff_wire: f64,
}

impl PositionTraceRow<'_> {
    pub fn format_csv_with_meta(&self, tick: u64, t_ms: u64) -> String {
        format!(
            "{tick},{t_ms},{joint},{q:.6},{dq:.6},{q_traj:.6},{dq_traj:.6},{q_des:.6},{target:.6},{target_raw:.6},{q_env_lo:.6},{q_env_hi:.6},{lead:.6},{lead_sat},{settle_error:.6},{phase},{friction_mode},{tau_p:.6},{tau_g:.6},{tau_f:.6},{tau_d:.6},{tau_ff_cmd:.6},{tau_meas:.6},{dq_mit:.6},{kp:.3},{kd:.3},{joint_stuck},{planner_frozen},{retarget_age_ms},{planner_event},{law},{q_ref:.6},{dq_ref:.6},{time_scale:.6},{tau_i:.6},{kd_mit:.3},{tau_ff_wire:.6}",
            tick = tick,
            t_ms = t_ms,
            joint = csv_escape(self.joint),
            q = self.q,
            dq = self.dq,
            q_traj = self.q_traj,
            dq_traj = self.dq_traj,
            q_des = self.q_des,
            target = self.target,
            target_raw = self.target_raw,
            q_env_lo = self.q_env_lo,
            q_env_hi = self.q_env_hi,
            lead = self.lead,
            lead_sat = if self.lead_sat { 1 } else { 0 },
            settle_error = self.settle_error,
            phase = csv_escape(self.phase),
            friction_mode = csv_escape(self.friction_mode),
            tau_p = self.tau_p,
            tau_g = self.tau_g,
            tau_f = self.tau_f,
            tau_d = self.tau_d,
            tau_ff_cmd = self.tau_ff_cmd,
            tau_meas = self.tau_meas,
            dq_mit = self.dq_mit,
            kp = self.kp,
            kd = self.kd,
            joint_stuck = if self.joint_stuck { 1 } else { 0 },
            planner_frozen = if self.planner_frozen { 1 } else { 0 },
            retarget_age_ms = self.retarget_age_ms,
            planner_event = csv_escape(self.planner_event),
            law = csv_escape(self.law),
            q_ref = self.q_ref,
            dq_ref = self.dq_ref,
            time_scale = self.time_scale,
            tau_i = self.tau_i,
            kd_mit = self.kd_mit,
            tau_ff_wire = self.tau_ff_wire,
        )
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') {
        format!("\"{s}\"")
    } else {
        s.to_string()
    }
}

static TRACE_INIT_LOGGED: OnceLock<()> = OnceLock::new();

pub fn log_trace_enabled_once(path: &Path) {
    TRACE_INIT_LOGGED.get_or_init(|| {
        tracing::info!(
            path = %path.display(),
            "position trace CSV enabled (MARENGO_POSITION_TRACE)"
        );
    });
}

#[cfg(test)]
mod tests {
    #![allow(clippy::approx_constant, clippy::expect_used)]

    use super::*;

    #[test]
    fn csv_row_includes_command_and_measured_torque() {
        let row = PositionTraceRow {
            joint: "right_shoulder_pitch",
            q: 0.5,
            dq: 0.1,
            q_traj: 0.48,
            dq_traj: 0.12,
            q_des: 0.52,
            target: 1.57,
            target_raw: 1.57,
            q_env_lo: -0.85,
            q_env_hi: 3.14,
            lead: 0.02,
            lead_sat: false,
            settle_error: 1.07,
            phase: "Cruise",
            friction_mode: "traj_vel",
            tau_p: 0.24,
            tau_g: 1.2,
            tau_f: 0.5,
            tau_d: 0.02,
            tau_ff_cmd: 1.74,
            tau_meas: 1.5,
            dq_mit: 0.12,
            kp: 12.0,
            kd: 1.0,
            joint_stuck: true,
            planner_frozen: false,
            retarget_age_ms: 42,
            planner_event: "tick",
            law: "scaled_pd",
            q_ref: 0.48,
            dq_ref: 0.12,
            time_scale: 0.5,
            tau_i: 0.01,
            kd_mit: 3.0,
            tau_ff_wire: 1.7,
        };
        let line = row.format_csv_with_meta(42, 1234);
        assert!(line.starts_with("42,1234,right_shoulder_pitch,"));
        assert!(line.contains(",1.740000,1.500000,0.120000,"));
        assert!(line.contains(",Cruise,traj_vel,"));
        assert!(line.ends_with(
            ",1,0,42,tick,scaled_pd,0.480000,0.120000,0.500000,0.010000,3.000,1.700000"
        ));
        let header_columns = 37;
        assert_eq!(line.split(',').count(), header_columns);
    }

    fn sample_row() -> PositionTraceRow<'static> {
        PositionTraceRow {
            joint: "right_shoulder_pitch",
            q: 0.5,
            dq: 0.1,
            q_traj: 0.48,
            dq_traj: 0.12,
            q_des: 0.52,
            target: 1.57,
            target_raw: 1.57,
            q_env_lo: -0.85,
            q_env_hi: 3.14,
            lead: 0.02,
            lead_sat: false,
            settle_error: 1.07,
            phase: "Cruise",
            friction_mode: "traj_vel",
            tau_p: 0.24,
            tau_g: 1.2,
            tau_f: 0.5,
            tau_d: 0.02,
            tau_ff_cmd: 1.74,
            tau_meas: 1.5,
            dq_mit: 0.12,
            kp: 12.0,
            kd: 1.0,
            joint_stuck: true,
            planner_frozen: false,
            retarget_age_ms: 42,
            planner_event: "tick",
            law: "legacy",
            q_ref: 0.48,
            dq_ref: 0.12,
            time_scale: 1.0,
            tau_i: 0.0,
            kd_mit: 0.0,
            tau_ff_wire: f64::NAN,
        }
    }

    #[test]
    fn healthy_trace_records_and_flushes() {
        let dir = std::env::temp_dir().join(format!("trace-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let path = dir.join("trace.csv");
        let row = sample_row();
        let mut trace = PositionTrace::open_for_test(&path, 200).expect("open");
        trace.maybe_record(0, 0, &row);
        trace.maybe_record(1, 5, &row);
        trace.flush().expect("flush");
        assert_eq!(trace.write_errors(), 0);
        let body = std::fs::read_to_string(&path).expect("readback");
        assert_eq!(body.lines().count(), 3, "header plus two rows");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn session_cap_stops_recording_without_error() {
        // L-berthier-07: an unattended trace stops at the cap instead of
        // growing the file without bound.
        let dir = std::env::temp_dir().join(format!("trace-cap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let path = dir.join("trace.csv");
        let row = sample_row();
        let mut trace =
            PositionTrace::open_inner(&path, 200, TRACE_BUFFER_BYTES, 10).expect("open");
        trace.maybe_record(0, 0, &row);
        trace.maybe_record(1, 5, &row);
        trace.flush().expect("flush");
        assert_eq!(trace.write_errors(), 0);
        let body = std::fs::read_to_string(&path).expect("readback");
        assert_eq!(body.lines().count(), 1, "header only; rows capped");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_refuses_already_huge_file() {
        let dir = std::env::temp_dir().join(format!("trace-huge-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let path = dir.join("trace.csv");
        std::fs::write(&path, vec![b'x'; 64]).expect("prefill");
        let err = PositionTrace::open_inner(&path, 200, TRACE_BUFFER_BYTES, 16)
            .expect_err("huge file refused");
        assert_eq!(err.kind(), std::io::ErrorKind::QuotaExceeded);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_surfaces_unwritable_file() {
        // L-berthier-07: open errors are returned (and warned in `from_env`),
        // never silently dropped. A directory is never writable as a file.
        let dir = std::env::temp_dir().join(format!("trace-dir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        PositionTrace::open_inner(&dir.join("missing-parent").join("t.csv"), 200, 1, u64::MAX)
            .expect_err("unwritable file refused");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_failure_disables_trace_and_counts_once() {
        // The first failed tick write is counted, and later ticks do not
        // retry into the error.
        let dir = std::env::temp_dir().join(format!("trace-err-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let path = dir.join("trace.csv");
        let mut trace = PositionTrace::open_for_test(&path, 200).expect("open");
        assert_eq!(trace.write_errors(), 0);
        trace.note_write_result(Err(std::io::Error::new(
            std::io::ErrorKind::StorageFull,
            "disk full",
        )));
        assert_eq!(trace.write_errors(), 1);
        let row = sample_row();
        trace.maybe_record(0, 0, &row);
        trace.maybe_record(1, 5, &row);
        assert_eq!(trace.write_errors(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
