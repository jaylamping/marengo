//! # robstride — Robstride CAN driver (MIT Mode 0)
//!
//! Hardware transport for RS00–RS04 actuators: encode/decode MIT frames, send/recv on CAN.
//! **No control policy** — only bytes on the bus and a feedback cache.
//!
//! ## Responsibilities
//!
//! - [`mit`](mit): pack/unpack MIT `{kp, kd, q, dq, tau_ff}` per [`MotorType`](marengo_config::MotorType).
//! - [`bus::MotorBus`]: `mit_control_all`, `recv_all`, extended 29-bit IDs (`0x200 + device_id`).
//! - [`state::MotorState`]: last `q`, `dq`, `tau`, fault per `device_id`.
//! - [`protocol`](protocol): legacy 11-bit stub (tests only; do not use on bench).
//! - Optional SocketCAN backend (`vcan` feature, Linux).
//!
//! ## Does not
//!
//! - Decide torque limits, enable motors, or handle E-stop (Davout).
//! - Run periodic control or gravity model (Berthier / armee-dynamics).
//! - Load YAML or URDF (marengo-config / armee-kinematics).
//!
//! ## Callers
//!
//! | Caller | Usage |
//! |--------|--------|
//! | Davout | Sole production path to `send_mit` / `MotorBus` |
//! | Tests / `motor-repl` | [`MemoryBus`](bus::MemoryBus) without hardware |
//!
//! Wire spec: [hardware/docs/decisions/0002-robstride-protocol.md](../../hardware/docs/decisions/0002-robstride-protocol.md).

pub mod bus;
pub mod mit;
pub mod motor_type;
pub mod protocol;
pub mod state;

pub use bus::{
    send_mit, send_motion, send_motion_legacy, BusError, CanBus, CanFrame, JointMotion, MemoryBus,
    MotorBus,
};
pub use mit::{encode_mit, mit_rx_id, mit_tx_id, MitCommand, MitFeedback};
pub use protocol::{
    command_can_id, decode_feedback, encode_command, MotionCommand, MotionFeedback,
};
pub use state::MotorState;

#[cfg(all(feature = "vcan", target_os = "linux"))]
pub mod vcan {
    //! Virtual CAN helpers for bench tests.

    /// Default SocketCAN interface used in compose `vcan` profile.
    pub const DEFAULT_INTERFACE: &str = "vcan0";
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::HashMap;
    use std::time::Duration;

    use marengo_config::MotorType;

    use super::bus::{MemoryBus, MotorBus};
    use super::mit::{encode_mit, mit_tx_id, MitCommand};

    #[test]
    fn encode_mit_extended_id() {
        let cmd = MitCommand {
            device_id: 2,
            motor_type: MotorType::Rs02,
            position_rad: 0.0,
            velocity_rad_s: 0.0,
            kp: 0.0,
            kd: 0.0,
            torque_ff_nm: 1.0,
        };
        let (id, _) = encode_mit(&cmd);
        assert_eq!(id, mit_tx_id(2));
    }

    #[test]
    fn memory_bus_mit_control_records_extended() {
        let mut bus = MemoryBus::default();
        let cmd = MitCommand {
            device_id: 1,
            motor_type: MotorType::Rs03,
            position_rad: 0.1,
            velocity_rad_s: 0.0,
            kp: 0.0,
            kd: 0.0,
            torque_ff_nm: 2.0,
        };
        bus.mit_control_all(&[cmd]).expect("send");
        assert_eq!(bus.tx.len(), 1);
        assert!(bus.tx[0].extended);
        assert_eq!(bus.tx[0].id, 0x201);
    }

    #[test]
    fn recv_all_times_out_without_rx() {
        let mut bus = MemoryBus::default();
        let mut states = HashMap::new();
        let types = HashMap::from([(1u8, MotorType::Rs03)]);
        let err = bus
            .recv_all(&types, &mut states, Duration::from_millis(5))
            .expect_err("timeout");
        assert!(matches!(err, super::bus::BusError::RecvTimeout));
    }
}
