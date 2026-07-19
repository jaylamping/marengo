//! Continuous cosine-wave position target for one joint (in-loop, no stdin pacing).

/// Oscillate one joint between `min_rad` and `max_rad` for `cycles` full periods.
///
/// Uses a raised cosine (smooth velocity at the endpoints) so the planner is not
/// chased by triangle corners — those caused choppy bench motion.
#[derive(Debug, Clone)]
pub struct PositionWave {
    pub joint_index: usize,
    min_rad: f64,
    max_rad: f64,
    start_tick: u64,
    half_period_ticks: u64,
    cycles: u32,
    finished: bool,
}

impl PositionWave {
    pub fn new(
        joint_index: usize,
        min_rad: f64,
        max_rad: f64,
        start_tick: u64,
        half_period_ticks: u64,
        cycles: u32,
    ) -> Self {
        Self {
            joint_index,
            min_rad,
            max_rad,
            start_tick,
            half_period_ticks: half_period_ticks.max(1),
            cycles,
            finished: false,
        }
    }

    /// Cosine setpoint + analytical velocity at `tick`, or `None` when complete.
    ///
    /// Phase 0 → `min_rad`, phase 0.5 → `max_rad`, phase 1 → `min_rad` (C¹ at endpoints).
    /// Velocity is `d/dt` of the cosine (zero at endpoints) — avoids noisy finite differences.
    pub fn target_and_velocity_at_tick(&mut self, tick: u64, loop_hz: u32) -> Option<(f64, f64)> {
        let full_period = self.half_period_ticks.saturating_mul(2);
        let elapsed = tick.saturating_sub(self.start_tick);
        let total_ticks = full_period.saturating_mul(u64::from(self.cycles));
        if elapsed >= total_ticks {
            self.finished = true;
            return None;
        }
        let phase = (elapsed % full_period) as f64 / full_period as f64;
        let mid = 0.5 * (self.min_rad + self.max_rad);
        let amp = 0.5 * (self.max_rad - self.min_rad);
        let angle = std::f64::consts::TAU * phase;
        let target = mid - amp * angle.cos();
        let period_sec = full_period as f64 / f64::from(loop_hz.max(1));
        let omega = std::f64::consts::TAU / period_sec.max(1e-9);
        // d/dt [mid - amp*cos(ωt)] = amp*ω*sin(ωt)
        let velocity = amp * omega * angle.sin();
        Some((target, velocity))
    }

    /// Cosine setpoint at `tick`, or `None` when all cycles are complete.
    #[cfg(test)]
    pub fn target_at_tick(&mut self, tick: u64) -> Option<f64> {
        // loop_hz only scales velocity; position phase is tick-based.
        self.target_and_velocity_at_tick(tick, 200).map(|(q, _)| q)
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn end_position_rad(&self) -> f64 {
        self.min_rad
    }

    pub fn duration_sec(&self, loop_hz: u32) -> f64 {
        let hz = f64::from(loop_hz.max(1));
        let ticks = self
            .half_period_ticks
            .saturating_mul(2)
            .saturating_mul(u64::from(self.cycles));
        ticks as f64 / hz
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::PositionWave;

    #[test]
    fn cosine_wave_endpoints_and_duration() {
        let mut wave = PositionWave::new(0, 0.4, 1.0, 0, 40, 2);
        assert!((wave.target_at_tick(0).unwrap() - 0.4).abs() < 1e-9);
        assert!((wave.target_at_tick(40).unwrap() - 1.0).abs() < 1e-9);
        assert!((wave.target_at_tick(80).unwrap() - 0.4).abs() < 1e-9);
        assert!(wave.target_at_tick(160).is_none());
        assert!(wave.is_finished());
        // 40 half * 2 * 2 cycles = 160 ticks @ 200 Hz → 0.8 s
        assert!((wave.duration_sec(200) - 0.8).abs() < 1e-9);
    }

    #[test]
    fn cosine_is_smooth_near_endpoints() {
        let mut wave = PositionWave::new(0, 0.0, 1.0, 0, 100, 1);
        let a = wave.target_at_tick(0).unwrap();
        let b = wave.target_at_tick(1).unwrap();
        let c = wave.target_at_tick(2).unwrap();
        // Near min, first differences shrink (zero derivative at endpoint).
        assert!((b - a).abs() < (c - b).abs() + 1e-12);
    }

    #[test]
    fn analytical_velocity_zero_at_endpoints() {
        let mut wave = PositionWave::new(0, 0.4, 0.8, 0, 100, 1);
        let (_, v0) = wave.target_and_velocity_at_tick(0, 200).unwrap();
        let (_, v_mid) = wave.target_and_velocity_at_tick(100, 200).unwrap();
        assert!(v0.abs() < 1e-9, "v at min must be 0, got {v0}");
        assert!(v_mid.abs() < 1e-9, "v at max must be 0, got {v_mid}");
        let (_, v_up) = {
            let mut w = PositionWave::new(0, 0.4, 0.8, 0, 100, 1);
            w.target_and_velocity_at_tick(50, 200).unwrap()
        };
        assert!(v_up > 0.0, "rising half should have +velocity, got {v_up}");
    }
}
