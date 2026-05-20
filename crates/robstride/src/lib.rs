//! Robstride RS-series motor protocol over CAN.
//!
//! All motion commands must be issued through [Davout](../davout/) — do not call this crate
//! directly from control code.

pub mod bus;
pub mod protocol;

pub use bus::{BusError, CanBus, JointMotion, MemoryBus, send_motion};
pub use protocol::{MotionCommand, MotionFeedback, command_can_id, decode_feedback, encode_command};

#[cfg(all(feature = "vcan", target_os = "linux"))]
pub mod vcan {
    //! Virtual CAN helpers for bench tests.

    /// Default SocketCAN interface used in compose `vcan` profile.
    pub const DEFAULT_INTERFACE: &str = "vcan0";
}

#[cfg(all(feature = "vcan", target_os = "linux", test))]
mod vcan_tests {
    #![allow(clippy::expect_used)]

    use super::bus::{JointMotion, MemoryBus, send_motion};
    use super::protocol::{command_can_id, encode_command};
    use super::vcan::DEFAULT_INTERFACE;
    use socketcan::{CanSocket, Socket};

    #[test]
    fn encode_command_produces_expected_id() {
        let cmd = super::MotionCommand {
            device_id: 1,
            position_rad: 0.1,
            velocity_rad_s: 0.0,
            torque_nm: 0.0,
        };
        let (id, _) = encode_command(&cmd);
        assert_eq!(id, command_can_id(1));
    }

    #[test]
    fn memory_bus_records_frame() {
        let mut bus = MemoryBus::default();
        send_motion(
            &mut bus,
            &JointMotion {
                joint: "joint1".to_string(),
                device_id: 1,
                position_rad: 0.2,
                velocity_rad_s: 0.1,
                torque_nm: 0.0,
            },
        )
        .expect("send");
        assert_eq!(bus.frames.len(), 1);
    }

    #[test]
    #[ignore = "requires vcan0 (docker compose --profile vcan)"]
    fn vcan0_send_smoke() {
        use super::bus::SocketCanBus;
        let mut bus = SocketCanBus::open(DEFAULT_INTERFACE).expect("open vcan0");
        send_motion(
            &mut bus,
            &JointMotion {
                joint: "joint1".to_string(),
                device_id: 1,
                position_rad: 0.0,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
            },
        )
        .expect("send on vcan");
    }

    #[test]
    #[ignore = "requires vcan0 (docker compose --profile vcan)"]
    fn vcan0_exists() {
        let socket = CanSocket::open(DEFAULT_INTERFACE)
            .expect("open vcan0 — run scripts/vcan-up.sh or compose profile vcan");
        drop(socket);
    }
}
