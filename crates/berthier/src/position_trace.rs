//! High-rate CSV trace for position-hold bench debugging (`MARENGO_POSITION_TRACE`).

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::OnceLock;

/// Comma-separated joints traced every tick regardless of `MARENGO_POSITION_TRACE_HZ`, so a
/// bench sweep can score per-tick quantities (τ_ff step) on its joint while the rest stay
/// decimated.
pub const FULL_RATE_JOINTS_ENV: &str = "MARENGO_POSITION_TRACE_FULL_RATE_JOINTS";

/// Buffered CSV writer for position-hold diagnostics (optional, env-gated).
#[derive(Debug)]
pub struct PositionTrace {
    writer: BufWriter<File>,
    period_ticks: u64,
    /// Per joint index: traced every tick (`FULL_RATE_JOINTS_ENV`) instead of every period.
    full_rate: Vec<bool>,
    /// Reused row buffer: formatting a row allocates nothing once it has grown to a row.
    line: String,
    byte_cap: u64,
    bytes_written: u64,
    failed: bool,
    write_errors: u64,
}

/// Session cap so an unattended trace cannot fill the disk (L-berthier-07).
/// At full 5-joint 200 Hz (~150 KB/s) this is roughly half an hour.
pub const TRACE_SESSION_CAP_BYTES: u64 = 256 * 1024 * 1024;

const TRACE_BUFFER_BYTES: usize = 512 * 1024;
/// Room for one row (about 300 bytes) without regrowing `line`.
const TRACE_LINE_CAPACITY: usize = 512;

impl PositionTrace {
    /// Open trace file when `MARENGO_POSITION_TRACE` is set to a writable path.
    ///
    /// Sample rate defaults to `loop_hz` unless `MARENGO_POSITION_TRACE_HZ` is set; joints named
    /// in `MARENGO_POSITION_TRACE_FULL_RATE_JOINTS` are sampled every tick.
    pub fn from_env(loop_hz: u32, joint_names: &[String]) -> Option<Self> {
        let path = std::env::var_os("MARENGO_POSITION_TRACE")?;
        let trace_hz = std::env::var("MARENGO_POSITION_TRACE_HZ")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|&hz| hz > 0)
            .unwrap_or(loop_hz);
        let full_rate = std::env::var(FULL_RATE_JOINTS_ENV)
            .map(|list| full_rate_mask(&list, joint_names))
            .unwrap_or_else(|_| vec![false; joint_names.len()]);
        match Self::open(Path::new(&path), loop_hz, trace_hz, full_rate) {
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

    fn open(
        path: &Path,
        loop_hz: u32,
        trace_hz: u32,
        full_rate: Vec<bool>,
    ) -> std::io::Result<Self> {
        Self::open_inner(
            path,
            loop_hz,
            trace_hz,
            full_rate,
            TRACE_BUFFER_BYTES,
            TRACE_SESSION_CAP_BYTES,
        )
    }

    fn open_inner(
        path: &Path,
        loop_hz: u32,
        trace_hz: u32,
        full_rate: Vec<bool>,
        buffer_bytes: usize,
        byte_cap: u64,
    ) -> std::io::Result<Self> {
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
            full_rate,
            line: String::with_capacity(TRACE_LINE_CAPACITY),
            byte_cap,
            bytes_written: 0,
            failed: false,
            write_errors: 0,
        })
    }

    #[cfg(test)]
    pub(crate) fn open_for_test(path: &Path, loop_hz: u32) -> std::io::Result<Self> {
        Self::open(path, loop_hz, loop_hz, Vec::new())
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

    /// Whether a row of joint `joint_index` at `tick` would be written: a full-rate joint, or
    /// `tick` on the decimation period. Lets the caller skip building rows that are dropped.
    pub fn records(&self, tick: u64, joint_index: usize) -> bool {
        let full_rate = self.full_rate.get(joint_index).copied().unwrap_or(false);
        !self.failed && (full_rate || tick % self.period_ticks == 0)
    }

    /// Record one sample of joint `joint_index` when [`Self::records`] holds.
    ///
    /// Infallible by design: a failed tick write disables tracing with one
    /// warning instead of erroring (or retry-failing) every tick.
    pub fn maybe_record(
        &mut self,
        tick: u64,
        t_ms: u64,
        joint_index: usize,
        row: &PositionTraceRow<'_>,
    ) {
        if !self.records(tick, joint_index) {
            return;
        }
        self.line.clear();
        // Writing into a String cannot fail.
        let _ = row.write_csv_with_meta(&mut self.line, tick, t_ms);
        self.line.push('\n');
        if self.bytes_written.saturating_add(self.line.len() as u64) > self.byte_cap {
            self.failed = true;
            tracing::warn!(
                cap_bytes = self.byte_cap,
                "position trace session cap reached; tracing disabled"
            );
            return;
        }
        let result = self.writer.write_all(self.line.as_bytes());
        self.note_write_result(result);
        if self.failed {
            return;
        }
        self.bytes_written = self.bytes_written.saturating_add(self.line.len() as u64);
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
    /// The row (no newline) written into `out`.
    pub fn write_csv_with_meta(
        &self,
        out: &mut impl fmt::Write,
        tick: u64,
        t_ms: u64,
    ) -> fmt::Result {
        write!(
            out,
            "{tick},{t_ms},{joint},{q:.6},{dq:.6},{q_traj:.6},{dq_traj:.6},{q_des:.6},{target:.6},{target_raw:.6},{q_env_lo:.6},{q_env_hi:.6},{lead:.6},{lead_sat},{settle_error:.6},{phase},{friction_mode},{tau_p:.6},{tau_g:.6},{tau_f:.6},{tau_d:.6},{tau_ff_cmd:.6},{tau_meas:.6},{dq_mit:.6},{kp:.3},{kd:.3},{joint_stuck},{planner_frozen},{retarget_age_ms},{planner_event},{law},{q_ref:.6},{dq_ref:.6},{time_scale:.6},{tau_i:.6},{kd_mit:.3},{tau_ff_wire:.6}",
            tick = tick,
            t_ms = t_ms,
            joint = CsvField(self.joint),
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
            lead_sat = u8::from(self.lead_sat),
            settle_error = self.settle_error,
            phase = CsvField(self.phase),
            friction_mode = CsvField(self.friction_mode),
            tau_p = self.tau_p,
            tau_g = self.tau_g,
            tau_f = self.tau_f,
            tau_d = self.tau_d,
            tau_ff_cmd = self.tau_ff_cmd,
            tau_meas = self.tau_meas,
            dq_mit = self.dq_mit,
            kp = self.kp,
            kd = self.kd,
            joint_stuck = u8::from(self.joint_stuck),
            planner_frozen = u8::from(self.planner_frozen),
            retarget_age_ms = self.retarget_age_ms,
            planner_event = CsvField(self.planner_event),
            law = CsvField(self.law),
            q_ref = self.q_ref,
            dq_ref = self.dq_ref,
            time_scale = self.time_scale,
            tau_i = self.tau_i,
            kd_mit = self.kd_mit,
            tau_ff_wire = self.tau_ff_wire,
        )
    }
}

/// A text field, double-quoted when it contains a comma (no allocation).
struct CsvField<'a>(&'a str);

impl fmt::Display for CsvField<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.contains(',') {
            write!(f, "\"{}\"", self.0)
        } else {
            f.write_str(self.0)
        }
    }
}

/// Per joint index: named in the comma-separated `list`. Unknown names are warned about and
/// ignored (the trace is a diagnostic; the scorer reports a joint it finds decimated).
fn full_rate_mask(list: &str, joint_names: &[String]) -> Vec<bool> {
    let mut mask = vec![false; joint_names.len()];
    for name in list.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        match joint_names.iter().position(|j| j == name) {
            Some(i) => mask[i] = true,
            None => tracing::warn!(
                joint = name,
                "{FULL_RATE_JOINTS_ENV}: unknown joint ignored"
            ),
        }
    }
    mask
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
        let mut line = String::new();
        row.write_csv_with_meta(&mut line, 42, 1234)
            .expect("format row");
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
        trace.maybe_record(0, 0, 0, &row);
        trace.maybe_record(1, 5, 0, &row);
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
            PositionTrace::open_inner(&path, 200, 200, Vec::new(), TRACE_BUFFER_BYTES, 10)
                .expect("open");
        trace.maybe_record(0, 0, 0, &row);
        trace.maybe_record(1, 5, 0, &row);
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
        let err = PositionTrace::open_inner(&path, 200, 200, Vec::new(), TRACE_BUFFER_BYTES, 16)
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
        PositionTrace::open_inner(
            &dir.join("missing-parent").join("t.csv"),
            200,
            200,
            Vec::new(),
            1,
            u64::MAX,
        )
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
        trace.maybe_record(0, 0, 0, &row);
        trace.maybe_record(1, 5, 0, &row);
        assert_eq!(trace.write_errors(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn full_rate_joint_records_every_tick_while_others_stay_decimated() {
        // A 50 Hz trace at 200 Hz decimates by 4; the swept joint must still get every tick so
        // the bench scorer can measure the per-tick τ_ff step.
        let dir = std::env::temp_dir().join(format!("trace-full-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let path = dir.join("trace.csv");
        let names = ["a".to_string(), "b".to_string()];
        let mask = full_rate_mask(" b ,unknown", &names);
        assert_eq!(mask, vec![false, true]);
        let mut trace = PositionTrace::open(&path, 200, 50, mask).expect("open");
        let mut row = sample_row();
        for tick in 0..8 {
            row.joint = "a";
            trace.maybe_record(tick, tick * 5, 0, &row);
            row.joint = "b";
            trace.maybe_record(tick, tick * 5, 1, &row);
        }
        trace.flush().expect("flush");
        let body = std::fs::read_to_string(&path).expect("readback");
        let ticks = |joint: &str| -> Vec<u64> {
            body.lines()
                .skip(1)
                .filter(|l| l.split(',').nth(2) == Some(joint))
                .map(|l| {
                    l.split(',')
                        .next()
                        .and_then(|t| t.parse().ok())
                        .expect("tick")
                })
                .collect()
        };
        assert_eq!(ticks("a"), vec![0, 4]);
        assert_eq!(ticks("b"), (0..8).collect::<Vec<_>>());
        std::fs::remove_dir_all(&dir).ok();
    }
}
