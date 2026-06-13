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
                "tick,t_ms,joint,q,dq,q_traj,dq_traj,q_des,target,target_raw,q_env_lo,q_env_hi,lead,lead_sat,settle_error,phase,friction_mode,tau_p,tau_g,tau_f,tau_d,tau_ff_cmd,tau_meas,dq_mit,kp,kd,joint_stuck,planner_frozen"
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
}

impl PositionTraceRow<'_> {
    pub fn format_csv_with_meta(&self, tick: u64, t_ms: u64) -> String {
        format!(
            "{tick},{t_ms},{joint},{q:.6},{dq:.6},{q_traj:.6},{dq_traj:.6},{q_des:.6},{target:.6},{target_raw:.6},{q_env_lo:.6},{q_env_hi:.6},{lead:.6},{lead_sat},{settle_error:.6},{phase},{friction_mode},{tau_p:.6},{tau_g:.6},{tau_f:.6},{tau_d:.6},{tau_ff_cmd:.6},{tau_meas:.6},{dq_mit:.6},{kp:.3},{kd:.3},{joint_stuck},{planner_frozen}",
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
    #![allow(clippy::approx_constant)]

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
        };
        let line = row.format_csv_with_meta(42, 1234);
        assert!(line.starts_with("42,1234,right_shoulder_pitch,"));
        assert!(line.contains(",1.740000,1.500000,0.120000,"));
        assert!(line.contains(",Cruise,traj_vel,"));
        assert!(line.ends_with(",1,0"));
    }
}
