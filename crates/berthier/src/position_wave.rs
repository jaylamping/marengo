//! Continuous triangle-wave position target for one joint (in-loop, no stdin pacing).

/// Oscillate one joint between `min_rad` and `max_rad` for `cycles` full periods.
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

    /// Triangle-wave setpoint at `tick`, or `None` when all cycles are complete.
    pub fn target_at_tick(&mut self, tick: u64) -> Option<f64> {
        let full_period = self.half_period_ticks.saturating_mul(2);
        let elapsed = tick.saturating_sub(self.start_tick);
        let total_ticks = full_period.saturating_mul(u64::from(self.cycles));
        if elapsed >= total_ticks {
            self.finished = true;
            return None;
        }
        let phase = elapsed % full_period;
        let t = if phase < self.half_period_ticks {
            phase as f64 / self.half_period_ticks as f64
        } else {
            let down = phase - self.half_period_ticks;
            1.0 - down as f64 / self.half_period_ticks as f64
        };
        Some(self.min_rad + t * (self.max_rad - self.min_rad))
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
mod tests {
    use super::PositionWave;

    #[test]
    fn triangle_wave_endpoints_and_duration() {
        let mut wave = PositionWave::new(0, 0.4, 1.0, 0, 40, 2);
        assert!((wave.target_at_tick(0).unwrap() - 0.4).abs() < 1e-9);
        assert!((wave.target_at_tick(40).unwrap() - 1.0).abs() < 1e-9);
        assert!((wave.target_at_tick(80).unwrap() - 0.4).abs() < 1e-9);
        assert!(wave.target_at_tick(160).is_none());
        assert!(wave.is_finished());
        assert!((wave.duration_sec(200) - 1.6).abs() < 1e-9);
    }
}
