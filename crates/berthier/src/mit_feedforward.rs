//! MIT feedforward — Active MIT packing for GravityComp / Impedance / TorqueOnly.
//!
//! ControlLoop / [`crate::gain_runtime::GainRuntime`] resolve wire gains; this
//! module packs `τ_ff` + pre-resolved wire kp/kd. Dynamics (`τ_g`) stay in
//! `armee-dynamics`. TorqueOnly currently aliases GravityComp (`τ_ff = τ_g`).

use davout::{ControlMode, MitJointCommand};
use marengo_config::FrictionGains;

use crate::friction::friction_torque;

/// Per-joint inputs for one compose tick.
///
/// Wire gains come from [`crate::gain_runtime::GainRuntime::resolve_all`];
/// compose does not apply override/ramp policy.
#[derive(Debug, Clone)]
pub struct MitFfJointIn {
    pub name: String,
    pub q: f64,
    pub dq: f64,
    pub tau_g: f64,
    pub friction: Option<FrictionGains>,
    /// Pre-resolved MIT wire kp (override > ramp > YAML).
    pub wire_kp: f64,
    /// Pre-resolved MIT wire kd.
    pub wire_kd: f64,
    /// Coulomb fc for impedance friction; `None` → use `friction.fc` if present.
    pub fc: Option<f64>,
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
            let (tau_ff, q_des, mit_velocity) = match mode {
                ControlMode::GravityComp | ControlMode::TorqueOnly => (j.tau_g, j.q, 0.0),
                ControlMode::Impedance => {
                    let tau_f = j
                        .friction
                        .as_ref()
                        .map(|f| {
                            let fc = j.fc.unwrap_or(f.fc);
                            friction_torque(j.dq, fc, f.fv, f.fo, f.k)
                        })
                        .unwrap_or(0.0);
                    (j.tau_g + tau_f, j.q, 0.0)
                }
                ControlMode::Position | ControlMode::Disabled => continue,
            };
            batch.push(MitJointCommand {
                joint: j.name.clone(),
                kp: j.wire_kp,
                kd: j.wire_kd,
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

    fn joint(name: &str, tau_g: f64) -> MitFfJointIn {
        MitFfJointIn {
            name: name.to_string(),
            q: 0.3,
            dq: 0.0,
            tau_g,
            friction: None,
            wire_kp: 0.0,
            wire_kd: 0.0,
            fc: None,
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
    fn gravity_comp_uses_pre_resolved_wire_gains() {
        let mut j = joint("j0", 0.5);
        j.wire_kp = 9.0;
        j.wire_kd = 1.5;
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
    fn impedance_uses_wire_and_fc() {
        let mut j = joint("j0", 0.2);
        j.friction = Some(FrictionGains {
            fc: 0.1,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        });
        j.wire_kp = 18.0;
        j.wire_kd = 3.0;
        let bare = MitFeedforward::compose(ControlMode::Impedance, &[j.clone()]);
        assert!((bare[0].kp - 18.0).abs() < 1e-12);
        assert!((bare[0].kd - 3.0).abs() < 1e-12);

        j.wire_kp = 12.0;
        j.wire_kd = 2.0;
        j.fc = Some(0.0);
        let ov = MitFeedforward::compose(ControlMode::Impedance, &[j]);
        assert!((ov[0].kp - 12.0).abs() < 1e-12);
        assert!((ov[0].kd - 2.0).abs() < 1e-12);
    }
}
