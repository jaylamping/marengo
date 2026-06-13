//! High-rate CSV trace for position-hold bench debugging (`MARENGO_POSITION_TRACE`).

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::OnceLock;

/// Buffered CSV writer for position-hold diagnostics (optional, env-gated).
pub struct PositionTrace {
    writer: BufWriter<File>,
    period_ticks: u64,
}

impl PositionTrace {
    /// Open trace file when `MARENGO_POSITION_TRACE` is set to a writable path.
    ///
    /// Sample rate defaults to `loop_hz` unless `MARENGO_POSITION_TRACE_HZ` is set.
    pub fn from_env(loop_hz: u32) -> Option<Self> {
        let path = std::env::var_os("MARENGO_POSITION_TRACE")?;
        Self::open(Path::new(&path), loop_hz).ok()
    }

    fn open(path: &Path, loop_hz: u32) -> std::io::Result<Self> {
        let trace_hz = std::env::var("MARENGO_POSITION_TRACE_HZ")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|&hz| hz > 0)
            .unwrap_or(loop_hz);
        let period_ticks = (u64::from(loop_hz.max(1)) / u64::from(trace_hz.max(1))).max(1);
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let is_new = file.metadata().map(|m| m.len() == 0).unwrap_or(true);
        let mut writer = BufWriter::new(file);
        if is_new {
            writeln!(
                writer,
                "tick,t_ms,joint,q,dq,dq_traj,q_des,target,lead,lead_sat,tracking_error,settle_error,dist_to_target,on_trajectory,phase,kp,kd,tau_p,tau_g,tau_f,tau_d,tau_ff_cmd,estimated_tau,tau_meas,tau_err,kp_mit,kd_mit"
            )?;
        }
        log_trace_enabled_once(path);
        Ok(Self {
            writer,
            period_ticks,
        })
    }

    /// Record one sample when `tick` matches the decimation period.
    pub fn maybe_record(
        &mut self,
        tick: u64,
        t_ms: u64,
        row: &PositionTraceRow<'_>,
    ) -> std::io::Result<()> {
        if tick % self.period_ticks != 0 {
            return Ok(());
        }
        writeln!(self.writer, "{}", row.format_csv_with_meta(tick, t_ms))?;
        Ok(())
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

/// One position-hold trace row (command vs measured).
pub struct PositionTraceRow<'a> {
    pub joint: &'a str,
    pub q: f64,
    pub dq: f64,
    pub dq_traj: f64,
    pub q_des: f64,
    pub target: f64,
    pub lead: f64,
    pub lead_sat: bool,
    pub tracking_error: f64,
    pub settle_error: f64,
    pub dist_to_target: f64,
    pub on_trajectory: bool,
    pub phase: &'a str,
    pub kp: f64,
    pub kd: f64,
    pub tau_p: f64,
    pub tau_g: f64,
    pub tau_f: f64,
    pub tau_d: f64,
    pub tau_ff_cmd: f64,
    pub estimated_tau: f64,
    pub tau_meas: f64,
    pub kp_mit: f64,
    pub kd_mit: f64,
}

impl PositionTraceRow<'_> {
    pub fn format_csv_with_meta(&self, tick: u64, t_ms: u64) -> String {
        let tau_err = self.tau_meas - self.tau_ff_cmd;
        format!(
            "{tick},{t_ms},{joint},{q:.6},{dq:.6},{dq_traj:.6},{q_des:.6},{target:.6},{lead:.6},{lead_sat},{tracking_error:.6},{settle_error:.6},{dist_to_target:.6},{on_trajectory},{phase},{kp:.3},{kd:.3},{tau_p:.6},{tau_g:.6},{tau_f:.6},{tau_d:.6},{tau_ff_cmd:.6},{estimated_tau:.6},{tau_meas:.6},{tau_err:.6},{kp_mit:.3},{kd_mit:.3}",
            tick = tick,
            t_ms = t_ms,
            joint = csv_escape(self.joint),
            q = self.q,
            dq = self.dq,
            dq_traj = self.dq_traj,
            q_des = self.q_des,
            target = self.target,
            lead = self.lead,
            lead_sat = if self.lead_sat { 1 } else { 0 },
            tracking_error = self.tracking_error,
            settle_error = self.settle_error,
            dist_to_target = self.dist_to_target,
            on_trajectory = if self.on_trajectory { 1 } else { 0 },
            phase = csv_escape(self.phase),
            kp = self.kp,
            kd = self.kd,
            tau_p = self.tau_p,
            tau_g = self.tau_g,
            tau_f = self.tau_f,
            tau_d = self.tau_d,
            tau_ff_cmd = self.tau_ff_cmd,
            estimated_tau = self.estimated_tau,
            tau_meas = self.tau_meas,
            tau_err = tau_err,
            kp_mit = self.kp_mit,
            kd_mit = self.kd_mit,
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
    use super::*;

    #[test]
    fn csv_row_includes_command_and_measured_torque() {
        let row = PositionTraceRow {
            joint: "right_shoulder_pitch",
            q: 0.5,
            dq: 0.1,
            dq_traj: 0.12,
            q_des: 0.52,
            target: 1.57,
            lead: 0.02,
            lead_sat: false,
            tracking_error: -0.01,
            settle_error: 1.07,
            dist_to_target: 1.05,
            on_trajectory: true,
            phase: "Cruise",
            kp: 12.0,
            kd: 1.0,
            tau_p: 0.24,
            tau_g: 1.2,
            tau_f: 0.5,
            tau_d: 0.02,
            tau_ff_cmd: 1.74,
            estimated_tau: 1.98,
            tau_meas: 1.5,
            kp_mit: 12.0,
            kd_mit: 0.0,
        };
        let line = row.format_csv_with_meta(42, 1234);
        assert!(line.starts_with("42,1234,right_shoulder_pitch,"));
        assert!(line.contains(",1.740000,1.500000,-0.240000,"));
        assert!(line.contains(",Cruise,"));
    }
}
