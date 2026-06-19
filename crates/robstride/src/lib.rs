//! # robstride — Robstride CAN driver (MIT Mode 0)
//!
//! Hardware transport for RS00–RS04 actuators: encode/decode MIT frames, send/recv on CAN.
//! **No control policy** — only bytes on the bus and a feedback cache.
//!
//! ## Responsibilities
//!
//! - [`comm`](comm): pack/unpack Robstride 29-bit communication-type IDs.
//! - [`mit`](mit): pack/unpack MIT `{kp, kd, q, dq, tau_ff}` per [`MotorType`](marengo_config::MotorType).
//! - [`bus::MotorBus`]: `mit_control_all`, lifecycle, parameter writes, status receive.
//! - [`params`](params): firmware `run_mode` and parameter read/write frames.
//! - [`lifecycle`](lifecycle): enable, disable, and set-zero frames.
//! - [`state::MotorState`]: last `q`, `dq`, `tau`, fault per `device_id`.
//! - [`protocol`](protocol): legacy 11-bit stub (tests only; do not use on bench).
//! - Optional SocketCAN backend (`socketcan` feature, Linux).
//!
//! ## Does not
//!
//! - Decide torque limits, enable motors, or handle E-stop (Davout).
//! - Apply joint sign, gearing, or URDF-axis conventions (`direction` / `gear_ratio` live in Davout).
//! - Run periodic control or gravity model (Berthier / armee-dynamics).
//! - Load YAML or URDF (marengo-config / armee-kinematics).
//!
//! ## Callers
//!
//! | Caller | Usage |
//! |--------|--------|
//! | Davout | Sole production path to `send_mit` / `MotorBus` |
//! | Tests | [`MemoryBus`](bus::MemoryBus) without hardware |
//!
//! Wire spec: [hardware/docs/decisions/0002-robstride-protocol.md](../../hardware/docs/decisions/0002-robstride-protocol.md).

pub mod bus;
pub mod comm;
pub mod lifecycle;
pub mod mit;
pub mod motor_type;
pub mod params;
pub mod protocol;
pub mod state;

pub use bus::{
    send_mit, send_motion, send_motion_legacy, AddressedMitCommand, BusError, CanBus, CanFrame,
    JointMotion, MemoryBus, MotorAddress, MotorBus, ReceivedCanFrame, RuntimeBus,
};
#[cfg(all(feature = "socketcan", target_os = "linux"))]
pub use bus::{SocketCanBus, SocketCanRouter};
pub use comm::{pack_ext_id, unpack_ext_id, CommunicationType, ExtendedId, DEFAULT_HOST_ID};
pub use lifecycle::{
    encode_active_reporting, encode_default_active_reporting, encode_default_disable,
    encode_default_enable, encode_default_set_zero_position, encode_disable, encode_enable,
    encode_set_zero_position,
};
pub use mit::{encode_mit, mit_rx_id, mit_tx_id, MitCommand, MitFeedback};
pub use params::{
    encode_current_ref, encode_position_ref, encode_read_parameter, encode_set_run_mode,
    encode_speed_ref, encode_write_parameter, ParameterId, ParameterValue, RunMode,
};
pub use protocol::{
    command_can_id, decode_feedback, encode_command, MotionCommand, MotionFeedback,
};
pub use state::MotorState;

#[cfg(all(feature = "socketcan", target_os = "linux"))]
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

    use super::bus::{AddressedMitCommand, MemoryBus, MotorAddress, MotorBus, ReceivedCanFrame};
    use super::comm::{pack_typed_ext_id, unpack_ext_id, CommunicationType, DEFAULT_HOST_ID};
    use super::mit::{encode_mit, MitCommand};
    use super::CanFrame;

    fn status_frame(device_id: u8) -> CanFrame {
        CanFrame {
            id: pack_typed_ext_id(
                CommunicationType::OperationStatus,
                u16::from(device_id),
                DEFAULT_HOST_ID,
            ),
            data: [0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0x00, 0xC8],
            extended: true,
        }
    }

    /// Returns scripted RX batches in order; empty batches simulate inter-frame gaps.
    struct ScriptBus {
        batches: Vec<Vec<CanFrame>>,
        index: usize,
    }

    impl ScriptBus {
        fn new(batches: Vec<Vec<CanFrame>>) -> Self {
            Self { batches, index: 0 }
        }
    }

    impl super::CanBus for ScriptBus {
        fn send_frame(&mut self, _frame: &CanFrame) -> Result<(), super::BusError> {
            Ok(())
        }

        fn recv_frames(&mut self, out: &mut Vec<CanFrame>) -> Result<(), super::BusError> {
            if self.index < self.batches.len() {
                out.extend(self.batches[self.index].iter().cloned());
                self.index += 1;
            }
            Ok(())
        }
    }

    impl MotorBus for ScriptBus {}

    #[derive(Default)]
    struct RoutedMemoryBus {
        tx: Vec<(MotorAddress, CanFrame)>,
        rx: Vec<ReceivedCanFrame>,
    }

    impl super::CanBus for RoutedMemoryBus {
        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), super::BusError> {
            self.tx.push((MotorAddress::new("can0", 0), frame.clone()));
            Ok(())
        }

        fn send_frame_to(
            &mut self,
            address: &MotorAddress,
            frame: &CanFrame,
        ) -> Result<(), super::BusError> {
            self.tx.push((address.clone(), frame.clone()));
            Ok(())
        }

        fn recv_frames_from(
            &mut self,
            out: &mut Vec<ReceivedCanFrame>,
        ) -> Result<(), super::BusError> {
            out.append(&mut self.rx);
            Ok(())
        }
    }

    impl MotorBus for RoutedMemoryBus {}

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
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(
            unpacked.comm_type,
            CommunicationType::OperationControl.as_u8()
        );
        assert_eq!(unpacked.device_id, 2);
        assert_ne!(unpacked.extra_data, 0x7FFF);
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
        let unpacked = unpack_ext_id(bus.tx[0].id).expect("extended id");
        assert_eq!(unpacked.device_id, 1);
        assert_eq!(
            unpacked.comm_type,
            CommunicationType::OperationControl.as_u8()
        );
    }

    #[test]
    fn addressed_mit_routes_to_configured_interface() {
        let mut bus = RoutedMemoryBus::default();
        let address = MotorAddress::new("can1", 1);
        let command = MitCommand {
            device_id: 1,
            motor_type: MotorType::Rs03,
            position_rad: 0.0,
            velocity_rad_s: 0.0,
            kp: 0.0,
            kd: 0.0,
            torque_ff_nm: 0.0,
        };
        bus.mit_control_all_at(&[AddressedMitCommand {
            address: address.clone(),
            command,
        }])
        .expect("send addressed");

        assert_eq!(bus.tx.len(), 1);
        assert_eq!(bus.tx[0].0, address);
        assert!(bus.tx[0].1.extended);
    }

    #[test]
    fn recv_all_times_out_without_rx() {
        let mut bus = MemoryBus::default();
        let mut states = HashMap::new();
        let types = HashMap::from([(1u8, MotorType::Rs03)]);
        let err = bus
            .recv_all(
                &types,
                &mut states,
                Duration::from_millis(5),
                Duration::from_micros(300),
            )
            .expect_err("timeout");
        assert!(matches!(err, super::bus::BusError::RecvTimeout));
    }

    #[test]
    fn recv_all_drains_multiple_status_frames() {
        let mut bus = MemoryBus::default();
        let status_data = [0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0x00, 0xC8];
        bus.rx_queue.push(CanFrame {
            id: pack_typed_ext_id(CommunicationType::OperationStatus, 1, DEFAULT_HOST_ID),
            data: status_data,
            extended: true,
        });
        bus.rx_queue.push(CanFrame {
            id: pack_typed_ext_id(CommunicationType::OperationStatus, 2, DEFAULT_HOST_ID),
            data: status_data,
            extended: true,
        });
        let mut states = HashMap::new();
        let types = HashMap::from([(1u8, MotorType::Rs03), (2u8, MotorType::Rs02)]);
        let count = bus
            .recv_all(
                &types,
                &mut states,
                Duration::from_millis(1),
                Duration::from_micros(300),
            )
            .expect("status frames");
        assert_eq!(count, 2);
        assert_eq!(states.len(), 2);
        assert!((states[&1].temperature_c - 20.0).abs() < 0.001);
    }

    #[test]
    fn recv_all_updates_fault_report() {
        let mut bus = MemoryBus::default();
        bus.rx_queue.push(CanFrame {
            id: pack_typed_ext_id(CommunicationType::FaultReport, 1, DEFAULT_HOST_ID),
            data: [0x34, 0x12, 0, 0, 0, 0, 0, 0],
            extended: true,
        });
        let mut states = HashMap::new();
        let types = HashMap::from([(1u8, MotorType::Rs03)]);
        let count = bus
            .recv_all(
                &types,
                &mut states,
                Duration::from_millis(1),
                Duration::from_micros(300),
            )
            .expect("fault report");
        assert_eq!(count, 1);
        assert_eq!(states[&1].fault, 0x1234);
    }

    #[test]
    fn recv_all_addressed_keeps_repeated_device_ids_separate() {
        let mut bus = RoutedMemoryBus::default();
        let can0_id1 = MotorAddress::new("can0", 1);
        let can1_id1 = MotorAddress::new("can1", 1);
        bus.rx.push(ReceivedCanFrame {
            interface: Some("can1".to_string()),
            frame: CanFrame {
                id: pack_typed_ext_id(CommunicationType::OperationStatus, 1, DEFAULT_HOST_ID),
                data: [0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0x00, 0xC8],
                extended: true,
            },
        });
        let mut states = HashMap::new();
        let types = HashMap::from([
            (can0_id1.clone(), MotorType::Rs03),
            (can1_id1.clone(), MotorType::Rs02),
        ]);

        let count = bus
            .recv_all_addressed(
                &types,
                &mut states,
                Duration::from_millis(1),
                Duration::from_micros(300),
            )
            .expect("addressed status");

        assert_eq!(count, 1);
        assert!(!states.contains_key(&can0_id1));
        assert!(states.contains_key(&can1_id1));
    }

    #[test]
    fn recv_all_addressed_drains_four_motor_burst_across_gaps() {
        let mut bus = ScriptBus::new(vec![
            vec![status_frame(1)],
            vec![],
            vec![status_frame(2)],
            vec![status_frame(3)],
            vec![status_frame(4)],
        ]);
        let mut states = HashMap::new();
        let types = HashMap::from([
            (MotorAddress::new("can0", 1), MotorType::Rs03),
            (MotorAddress::new("can0", 2), MotorType::Rs02),
            (MotorAddress::new("can0", 3), MotorType::Rs03),
            (MotorAddress::new("can0", 4), MotorType::Rs02),
        ]);
        let count = bus
            .recv_all_addressed(
                &types,
                &mut states,
                Duration::from_millis(10),
                Duration::from_micros(200),
            )
            .expect("four-motor burst");
        assert_eq!(count, 4);
        assert_eq!(states.len(), 4);
    }

    #[test]
    fn recv_all_stops_when_budget_exhausted_before_quiet() {
        let mut bus = ScriptBus::new((0..64).map(|id| vec![status_frame(id % 8 + 1)]).collect());
        let mut states = HashMap::new();
        let types = HashMap::from([(1u8, MotorType::Rs03)]);
        let started = std::time::Instant::now();
        let count = bus
            .recv_all(
                &types,
                &mut states,
                Duration::from_millis(2),
                Duration::from_millis(50),
            )
            .expect("budget-limited drain");
        let elapsed = started.elapsed();
        assert!(count > 0);
        assert!(count < 64);
        assert!(elapsed >= Duration::from_millis(2));
        assert!(elapsed < Duration::from_millis(20));
    }

    #[cfg(all(feature = "socketcan", target_os = "linux"))]
    #[test]
    #[ignore = "requires vcan0/vcan1 from scripts/vcan-up.sh or just vcan"]
    fn socketcan_routes_multiple_test_interfaces() {
        use super::CanBus;
        use marengo_config::{MotorBenchLimits, MotorEntry, MotorsConfigFile};
        use std::thread;

        let motors = MotorsConfigFile {
            motors: vec![
                MotorEntry {
                    joint: "left_test".to_string(),
                    driver: "robstride".to_string(),
                    motor_type: MotorType::Rs03,
                    can_interface: super::vcan::DEFAULT_INTERFACE.to_string(),
                    device_id: 1,
                    direction: 1,
                    gear_ratio: 1.0,
                    recv_can_id: 0,
                    firmware_version: "test".to_string(),
                    bench: MotorBenchLimits {
                        position_lower_rad: -1.0,
                        position_upper_rad: 1.0,
                        velocity_limit_rad_s: 1.0,
                        torque_limit_nm: 1.0,
                    },
                },
                MotorEntry {
                    joint: "right_test".to_string(),
                    driver: "robstride".to_string(),
                    motor_type: MotorType::Rs03,
                    can_interface: "vcan1".to_string(),
                    device_id: 1,
                    direction: 1,
                    gear_ratio: 1.0,
                    recv_can_id: 0,
                    firmware_version: "test".to_string(),
                    bench: MotorBenchLimits {
                        position_lower_rad: -1.0,
                        position_upper_rad: 1.0,
                        velocity_limit_rad_s: 1.0,
                        torque_limit_nm: 1.0,
                    },
                },
            ],
        };
        let mut router = super::SocketCanRouter::open(&motors).expect("open vcan router");
        let cmd = MitCommand {
            device_id: 1,
            motor_type: MotorType::Rs03,
            position_rad: 0.0,
            velocity_rad_s: 0.0,
            kp: 0.0,
            kd: 0.0,
            torque_ff_nm: 0.0,
        };
        router
            .mit_control_all_at(&[
                AddressedMitCommand {
                    address: MotorAddress::new(super::vcan::DEFAULT_INTERFACE, 1),
                    command: cmd,
                },
                AddressedMitCommand {
                    address: MotorAddress::new("vcan1", 1),
                    command: cmd,
                },
            ])
            .expect("send routed vcan frames");
        let expected_id = encode_mit(&cmd).0;
        let deadline = std::time::Instant::now() + Duration::from_millis(100);
        let mut frames: Vec<ReceivedCanFrame> = Vec::new();
        while std::time::Instant::now() < deadline {
            router
                .recv_frames_from(&mut frames)
                .expect("recv routed vcan frames");
            let saw_vcan0 = frames.iter().any(|frame| {
                frame.interface.as_deref() == Some(super::vcan::DEFAULT_INTERFACE)
                    && frame.frame.id == expected_id
            });
            let saw_vcan1 = frames.iter().any(|frame| {
                frame.interface.as_deref() == Some("vcan1") && frame.frame.id == expected_id
            });
            if saw_vcan0 && saw_vcan1 {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert!(
            frames.iter().any(|frame| {
                frame.interface.as_deref() == Some(super::vcan::DEFAULT_INTERFACE)
                    && frame.frame.id == expected_id
            }) && frames.iter().any(|frame| {
                frame.interface.as_deref() == Some("vcan1") && frame.frame.id == expected_id
            }),
            "did not receive MIT frames on both vcan test interfaces"
        );
    }
}
