//! GainRuntime — Testing overrides + mode-transition ramp + per-tick resolve.
//!
//! Owns sticky per-joint [`GainOverride`] state and the kp/kd ramp. Control law
//! modules ([`crate::position_hold`], [`crate::mit_feedforward`]) consume
//! [`ResolvedGains`] only; they do not own override/ramp state.

use std::collections::HashMap;

use davout::ControlMode;
use marengo_config::ModeGains;
use tracing::debug;

/// Per-joint runtime gain override for Testing page.
///
/// Values are clamped to [`GainClampLimits`] on apply (not on resolve).
#[derive(Debug, Clone, PartialEq)]
pub struct GainOverride {
    pub kp: f64,
    pub kd: f64,
    pub ki: f64,
    pub fc: f64,
}

/// Motor-type safety ceilings passed by the caller at apply time.
///
/// ControlLoop resolves these from `MotorTypeDefaults`; GainRuntime does not
/// look up config.
#[derive(Debug, Clone, Copy)]
pub struct GainClampLimits {
    pub kp_max: f64,
    pub kd_max: f64,
    pub tau_ff_max: f64,
}

/// YAML mode-gain pair for one joint (parallel to joint_names).
#[derive(Debug, Clone, Copy)]
pub struct JointModeGains<'a> {
    pub gravity_comp: &'a ModeGains,
    pub impedance: &'a ModeGains,
}

/// Per-joint resolved gains for one tick.
///
/// `law_*` feeds PositionHold / Impedance friction. `wire_kp` / `wire_kd` feed
/// the MIT bus. Position: wire may scale kp only; bus kd stays compose's
/// `kd_mit` (caller ignores `wire_kd` on the Position path). Ramp does **not**
/// enter HoldJointParams law fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedGains {
    pub law_kp: f64,
    pub law_kd: f64,
    pub law_ki: f64,
    /// Coulomb fc when sticky override present; else `None` (caller uses YAML fc).
    pub law_fc: Option<f64>,
    pub wire_kp: f64,
    pub wire_kd: f64,
}

/// Linear kp/kd ramp across mode transitions (~100 ms at 200 Hz).
#[derive(Debug, Clone)]
struct GainRamp {
    /// Per-joint: (from_kp, from_kd, to_kp, to_kd), joint_names order.
    joints: Vec<(f64, f64, f64, f64)>,
    ticks_remaining: u32,
    total_ticks: u32,
}

/// Owns override map + optional ramp; single resolve path for both Position and Mit FF.
#[derive(Debug, Default)]
pub struct GainRuntime {
    overrides: HashMap<String, GainOverride>,
    ramp: Option<GainRamp>,
}

impl GainRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, joint_name: &str) -> Option<&GainOverride> {
        self.overrides.get(joint_name)
    }

    /// Apply one override. No-op when `!mode_allows_gain_override(mode)`.
    /// Clamps to `limits` before insert (`ki` ceiling = `kp_max`).
    pub fn apply(
        &mut self,
        mode: ControlMode,
        joint_name: &str,
        gain_override: GainOverride,
        limits: GainClampLimits,
    ) {
        if !mode_allows_gain_override(mode) {
            debug!(
                ?mode,
                joint = joint_name,
                "ignore gain override in this control mode"
            );
            return;
        }
        let clamped = clamp_override(joint_name, gain_override, limits);
        self.overrides.insert(joint_name.to_string(), clamped);
    }

    /// Batch apply. Same mode gate as [`Self::apply`].
    pub fn apply_batch(
        &mut self,
        mode: ControlMode,
        overrides: &HashMap<String, GainOverride>,
        limits_for: impl Fn(&str) -> GainClampLimits,
    ) {
        if !mode_allows_gain_override(mode) {
            debug!(?mode, "ignore batch gain overrides in this control mode");
            return;
        }
        for (joint, ov) in overrides {
            let clamped = clamp_override(joint, ov.clone(), limits_for(joint));
            self.overrides.insert(joint.clone(), clamped);
        }
    }

    pub fn clear(&mut self, joint_name: &str) {
        self.overrides.remove(joint_name);
    }

    pub fn clear_all(&mut self) {
        self.overrides.clear();
    }

    /// Mode enter: clear sticky overrides when policy says so; arm kp/kd ramp
    /// for non-Disabled ↔ non-Disabled transitions (20 ticks).
    ///
    /// `from_gains` / `to_gains` are parallel to joint order (caller supplies
    /// current effective and YAML targets so ControlLoop keeps tau_ff seeding).
    pub fn on_mode_enter(
        &mut self,
        previous: ControlMode,
        next: ControlMode,
        from_gains: &[(f64, f64)],
        to_gains: &[(f64, f64)],
    ) {
        if mode_clears_gain_overrides_on_enter(next) {
            self.overrides.clear();
        }
        if next != ControlMode::Disabled && previous != ControlMode::Disabled {
            let n = from_gains.len().min(to_gains.len());
            self.ramp = Some(GainRamp {
                joints: (0..n)
                    .map(|i| {
                        let (from_kp, from_kd) = from_gains[i];
                        let (to_kp, to_kd) = to_gains[i];
                        (from_kp, from_kd, to_kp, to_kd)
                    })
                    .collect(),
                ticks_remaining: 20,
                total_ticks: 20,
            });
        }
    }

    /// Interpolated ramp (kp, kd) per joint, or `None` when no ramp.
    pub fn ramp_gains(&self) -> Option<Vec<(f64, f64)>> {
        let ramp = self.ramp.as_ref()?;
        let progress = ramp_progress(ramp);
        Some(
            ramp.joints
                .iter()
                .map(|(from_kp, from_kd, to_kp, to_kd)| {
                    let kp = from_kp + (to_kp - from_kp) * progress;
                    let kd = from_kd + (to_kd - from_kd) * progress;
                    (kp, kd)
                })
                .collect(),
        )
    }

    /// Effective per-joint (kp, kd): interpolated ramp if active, else `targets`.
    pub fn current_effective_gains(&self, targets: &[(f64, f64)]) -> Vec<(f64, f64)> {
        self.ramp_gains()
            .unwrap_or_else(|| targets.to_vec())
    }

    /// Decrement ramp after MIT send; clear when finished.
    pub fn advance_tick(&mut self) {
        if let Some(ref mut ramp) = self.ramp {
            ramp.ticks_remaining = ramp.ticks_remaining.saturating_sub(1);
            if ramp.ticks_remaining == 0 {
                self.ramp = None;
            }
        }
    }

    /// One resolve for every joint in `joint_names` order.
    ///
    /// `yaml.len()` must equal `joint_names.len()`. Law gains ignore ramp;
    /// wire gains use override (if mode allows) > ramp > YAML target.
    pub fn resolve_all(
        &self,
        mode: ControlMode,
        joint_names: &[String],
        yaml: &[JointModeGains<'_>],
    ) -> Vec<ResolvedGains> {
        let ramp = self.ramp_gains();
        joint_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let y = yaml.get(i).copied().unwrap_or(JointModeGains {
                    gravity_comp: &ZERO_GAINS,
                    impedance: &ZERO_GAINS,
                });
                let (target_kp, target_kd) =
                    target_gains_from_yaml(mode, y.gravity_comp, y.impedance);
                let ov = if mode_allows_gain_override(mode) {
                    self.overrides.get(name)
                } else {
                    None
                };
                let (ramp_kp, ramp_kd) = match ramp.as_ref() {
                    Some(r) if i < r.len() => (Some(r[i].0), Some(r[i].1)),
                    _ => (None, None),
                };
                let (wire_kp, wire_kd) = effective_wire_gains(
                    mode,
                    target_kp,
                    target_kd,
                    ov.map(|o| (o.kp, o.kd)),
                    ramp_kp,
                    ramp_kd,
                );
                // Law: override OR impedance YAML (never ramp). GravityComp etc.
                // still expose impedance fields; callers pick the right path.
                let (law_kp, law_kd, law_ki, law_fc) = match ov {
                    Some(o) => (o.kp, o.kd, o.ki, Some(o.fc)),
                    None => (y.impedance.kp, y.impedance.kd, y.impedance.ki, None),
                };
                ResolvedGains {
                    law_kp,
                    law_kd,
                    law_ki,
                    law_fc,
                    wire_kp,
                    wire_kd,
                }
            })
            .collect()
    }
}

const ZERO_GAINS: ModeGains = ModeGains {
    kp: 0.0,
    kd: 0.0,
    ki: 0.0,
};

fn ramp_progress(ramp: &GainRamp) -> f64 {
    1.0 - (f64::from(ramp.ticks_remaining) / f64::from(ramp.total_ticks))
}

fn clamp_override(joint_name: &str, ov: GainOverride, limits: GainClampLimits) -> GainOverride {
    let mut clamped = ov;
    if clamped.kp > limits.kp_max {
        tracing::warn!(
            joint = %joint_name,
            kp = clamped.kp,
            kp_max = limits.kp_max,
            "clamping kp override to kp_max"
        );
        clamped.kp = limits.kp_max;
    }
    if clamped.kd > limits.kd_max {
        tracing::warn!(
            joint = %joint_name,
            kd = clamped.kd,
            kd_max = limits.kd_max,
            "clamping kd override to kd_max"
        );
        clamped.kd = limits.kd_max;
    }
    // No ki_max in MotorTypeDefaults; conservative: ki ≤ kp_max.
    if clamped.ki > limits.kp_max {
        tracing::warn!(
            joint = %joint_name,
            ki = clamped.ki,
            kp_max = limits.kp_max,
            "clamping ki override to kp_max (conservative)"
        );
        clamped.ki = limits.kp_max;
    }
    if clamped.fc > limits.tau_ff_max {
        tracing::warn!(
            joint = %joint_name,
            fc = clamped.fc,
            tau_ff_max = limits.tau_ff_max,
            "clamping fc override to tau_ff_max_nm"
        );
        clamped.fc = limits.tau_ff_max;
    }
    clamped
}

/// Whether Testing gain overrides may affect wire / law for this mode.
pub fn mode_allows_gain_override(mode: ControlMode) -> bool {
    matches!(mode, ControlMode::Impedance | ControlMode::Position)
}

/// Sticky overrides must be cleared when entering these modes.
pub fn mode_clears_gain_overrides_on_enter(mode: ControlMode) -> bool {
    matches!(
        mode,
        ControlMode::GravityComp | ControlMode::TorqueOnly | ControlMode::Disabled
    )
}

/// YAML target (kp, kd) for a control mode (ModeGainPolicy).
pub fn target_gains_from_yaml(
    mode: ControlMode,
    gravity_comp: &ModeGains,
    impedance: &ModeGains,
) -> (f64, f64) {
    match mode {
        ControlMode::GravityComp | ControlMode::TorqueOnly | ControlMode::Disabled => {
            (gravity_comp.kp, gravity_comp.kd)
        }
        ControlMode::Impedance | ControlMode::Position => (impedance.kp, impedance.kd),
    }
}

/// Effective wire (kp, kd): YAML target, optional override, optional ramp.
///
/// GravityComp / TorqueOnly / Disabled ignore Testing overrides but still accept
/// an active mode-transition ramp.
pub fn effective_wire_gains(
    mode: ControlMode,
    target_kp: f64,
    target_kd: f64,
    override_kp_kd: Option<(f64, f64)>,
    ramp_kp: Option<f64>,
    ramp_kd: Option<f64>,
) -> (f64, f64) {
    if mode_allows_gain_override(mode) {
        if let Some((kp, kd)) = override_kp_kd {
            return (kp, kd);
        }
    }
    match (ramp_kp, ramp_kd) {
        (Some(kp), Some(kd)) => (kp, kd),
        _ => (target_kp, target_kd),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Characterization pin: override beats ramp; ramp beats YAML; G-comp ignores override.
    #[test]
    fn effective_wire_gains_precedence_contract() {
        let (kp, kd) = effective_wire_gains(
            ControlMode::Impedance,
            18.0,
            3.0,
            Some((40.0, 4.0)),
            Some(9.0),
            Some(1.5),
        );
        assert!((kp - 40.0).abs() < 1e-12);
        assert!((kd - 4.0).abs() < 1e-12);

        let (kp, kd) =
            effective_wire_gains(ControlMode::Impedance, 18.0, 3.0, None, Some(9.0), Some(1.5));
        assert!((kp - 9.0).abs() < 1e-12);
        assert!((kd - 1.5).abs() < 1e-12);

        let (kp, kd) = effective_wire_gains(ControlMode::Impedance, 18.0, 3.0, None, None, None);
        assert!((kp - 18.0).abs() < 1e-12);
        assert!((kd - 3.0).abs() < 1e-12);

        let (kp, kd) = effective_wire_gains(
            ControlMode::GravityComp,
            0.0,
            0.0,
            Some((40.0, 4.0)),
            Some(9.0),
            Some(1.5),
        );
        assert!((kp - 9.0).abs() < 1e-12);
        assert!((kd - 1.5).abs() < 1e-12);

        assert!(mode_allows_gain_override(ControlMode::Impedance));
        assert!(mode_allows_gain_override(ControlMode::Position));
        assert!(!mode_allows_gain_override(ControlMode::GravityComp));
        assert!(mode_clears_gain_overrides_on_enter(ControlMode::GravityComp));
        assert!(!mode_clears_gain_overrides_on_enter(ControlMode::Position));
    }
}
