//! MIT feedforward — Active MIT packing for GravityComp / Impedance / TorqueOnly.
//!
//! ControlLoop owns gain ramp + Testing overrides; this module applies
//! [`ModeGainPolicy`] and composes `τ_ff` + wire gains. Dynamics (`τ_g`) stay in
//! `armee-dynamics`. TorqueOnly currently aliases GravityComp (`τ_ff = τ_g`).

use davout::{ControlMode, MitJointCommand};
use marengo_config::{FrictionGains, ModeGains};

use crate::friction::friction_torque;

/// Runtime Testing override fields used by MIT feedforward (kp/kd/fc).
#[derive(Debug, Clone, Copy)]
pub struct MitFfOverride {
    pub kp: f64,
    pub kd: f64,
    pub fc: f64,
}

/// Per-joint inputs for one compose tick.
#[derive(Debug, Clone)]
pub struct MitFfJointIn {
    pub name: String,
    pub q: f64,
    pub dq: f64,
    pub tau_g: f64,
    pub gravity_comp: ModeGains,
    pub impedance: ModeGains,
    pub friction: Option<FrictionGains>,
    pub override_gains: Option<MitFfOverride>,
    /// Interpolated ramp kp when a ControlLoop gain ramp is active.
    pub ramp_kp: Option<f64>,
    pub ramp_kd: Option<f64>,
}

/// Whether Testing gain overrides may affect wire kp/kd for this mode.
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

/// Effective wire (kp, kd): YAML target, optional override (Impedance / Position), optional ramp.
///
/// GravityComp / TorqueOnly / Disabled ignore Testing overrides (ADR 0004 hard-zero path)
/// but still accept an active mode-transition ramp so kp/kd can slew toward the YAML zeros.
/// Callers should not *store* overrides in those modes (`mode_allows_gain_override`).
pub fn effective_wire_gains(
    mode: ControlMode,
    target_kp: f64,
    target_kd: f64,
    override_gains: Option<MitFfOverride>,
    ramp_kp: Option<f64>,
    ramp_kd: Option<f64>,
) -> (f64, f64) {
    if mode_allows_gain_override(mode) {
        if let Some(ov) = override_gains {
            return (ov.kp, ov.kd);
        }
    }
    match (ramp_kp, ramp_kd) {
        (Some(kp), Some(kd)) => (kp, kd),
        _ => (target_kp, target_kd),
    }
}

/// Stateless compose helper (mirrors [`crate::position_hold::PositionHold`] hybrid wiring).
#[derive(Debug, Default)]
pub struct MitFeedforward;

impl MitFeedforward {
    /// Pack MIT commands for GravityComp / Impedance / TorqueOnly.
    ///
    /// Returns an empty batch for Position / Disabled (caller must not use this path).
    pub fn compose(mode: ControlMode, joints: &[MitFfJointIn]) -> Vec<MitJointCommand> {
        if matches!(mode, ControlMode::Position | ControlMode::Disabled) {
            return Vec::new();
        }
        let mut batch = Vec::with_capacity(joints.len());
        for j in joints {
            let (target_kp, target_kd) =
                target_gains_from_yaml(mode, &j.gravity_comp, &j.impedance);
            let (kp, kd) = effective_wire_gains(
                mode,
                target_kp,
                target_kd,
                j.override_gains,
                j.ramp_kp,
                j.ramp_kd,
            );
            let (tau_ff, q_des, mit_velocity) = match mode {
                ControlMode::GravityComp | ControlMode::TorqueOnly => (j.tau_g, j.q, 0.0),
                ControlMode::Impedance => {
                    let tau_f = j
                        .friction
                        .as_ref()
                        .map(|f| {
                            let fc = j.override_gains.map(|ov| ov.fc).unwrap_or(f.fc);
                            friction_torque(j.dq, fc, f.fv, f.fo, f.k)
                        })
                        .unwrap_or(0.0);
                    (j.tau_g + tau_f, j.q, 0.0)
                }
                ControlMode::Position | ControlMode::Disabled => continue,
            };
            batch.push(MitJointCommand {
                joint: j.name.clone(),
                kp,
                kd,
                position_rad: q_des,
                velocity_rad_s: mit_velocity,
                torque_ff_nm: tau_ff,
            });
        }
        batch
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn zeros() -> ModeGains {
        ModeGains {
            kp: 0.0,
            kd: 0.0,
            ki: 0.0,
        }
    }

    fn impedance_gains() -> ModeGains {
        ModeGains {
            kp: 18.0,
            kd: 3.0,
            ki: 5.0,
        }
    }

    fn joint(name: &str, tau_g: f64) -> MitFfJointIn {
        MitFfJointIn {
            name: name.to_string(),
            q: 0.3,
            dq: 0.0,
            tau_g,
            gravity_comp: zeros(),
            impedance: impedance_gains(),
            friction: None,
            override_gains: None,
            ramp_kp: None,
            ramp_kd: None,
        }
    }

    #[test]
    fn gravity_comp_packs_tau_g_and_yaml_zeros() {
        let out = MitFeedforward::compose(ControlMode::GravityComp, &[joint("j0", 1.25)]);
        assert_eq!(out.len(), 1);
        assert!((out[0].torque_ff_nm - 1.25).abs() < 1e-12);
        assert!((out[0].kp).abs() < 1e-12);
        assert!((out[0].kd).abs() < 1e-12);
        assert!((out[0].position_rad - 0.3).abs() < 1e-12);
        assert!((out[0].velocity_rad_s).abs() < 1e-12);
    }

    #[test]
    fn gravity_comp_ignores_testing_override() {
        let mut j = joint("j0", 0.5);
        j.override_gains = Some(MitFfOverride {
            kp: 50.0,
            kd: 5.0,
            fc: 1.0,
        });
        let out = MitFeedforward::compose(ControlMode::GravityComp, &[j]);
        assert!((out[0].kp).abs() < 1e-12);
        assert!((out[0].kd).abs() < 1e-12);
    }

    #[test]
    fn gravity_comp_still_accepts_mode_ramp() {
        let mut j = joint("j0", 0.5);
        j.ramp_kp = Some(9.0);
        j.ramp_kd = Some(1.5);
        let out = MitFeedforward::compose(ControlMode::GravityComp, &[j]);
        assert!((out[0].kp - 9.0).abs() < 1e-12);
        assert!((out[0].kd - 1.5).abs() < 1e-12);
    }

    #[test]
    fn torque_only_aliases_gravity_comp() {
        let g = MitFeedforward::compose(ControlMode::GravityComp, &[joint("j0", 0.8)]);
        let t = MitFeedforward::compose(ControlMode::TorqueOnly, &[joint("j0", 0.8)]);
        assert_eq!(g[0].kp, t[0].kp);
        assert_eq!(g[0].kd, t[0].kd);
        assert!((g[0].torque_ff_nm - t[0].torque_ff_nm).abs() < 1e-12);
    }

    #[test]
    fn impedance_uses_yaml_and_override() {
        let mut j = joint("j0", 0.2);
        j.friction = Some(FrictionGains {
            fc: 0.1,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        });
        let bare = MitFeedforward::compose(ControlMode::Impedance, &[j.clone()]);
        assert!((bare[0].kp - 18.0).abs() < 1e-12);
        assert!((bare[0].kd - 3.0).abs() < 1e-12);

        j.override_gains = Some(MitFfOverride {
            kp: 12.0,
            kd: 2.0,
            fc: 0.0,
        });
        let ov = MitFeedforward::compose(ControlMode::Impedance, &[j]);
        assert!((ov[0].kp - 12.0).abs() < 1e-12);
        assert!((ov[0].kd - 2.0).abs() < 1e-12);
    }
}
