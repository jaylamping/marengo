//! # robstride — Robstride CAN driver (MIT Mode 0)
//!
//! Hardware transport for RS00–RS04 actuators: encode/decode MIT frames, send/recv on CAN.
//! **No control policy** — checked numeric encoding, bytes on the bus and ordered receive evidence.
//!
//! ## Responsibilities
//!
//! - [`comm`](comm): pack/unpack Robstride 29-bit communication-type IDs.
//! - [`mit`](mit): pack/unpack MIT `{kp, kd, q, dq, tau_ff}` per [`MotorType`](marengo_config::MotorType).
//! - [`bus::MotorBus`]: addressed MIT commands, lifecycle, parameters, status receive.
//! - [`params`](params): firmware `run_mode` and parameter read/write frames.
//! - [`command`](command): typed rejection of nonfinite input, negative gains and wrong register types.
//! - [`lifecycle`](lifecycle): enable, disable, and set-zero frames.
//! - [`identity`](identity): type-0 device-ID request and 64-bit MCU UID reply decoding.
//! - [`version`](version): type-4 `C4` firmware version query and its type-2 reply, decoded
//!   apart from MIT poses (header flags/mode retained, payload never a position).
//! - [`feedback`]: addressed observations retain status flags, drive mode and complete raw
//!   detailed-fault/warning payloads, malformed prefixes and transport errors in raw delivery order.
//! - [`receive`]: one shared nonblocking engine caps every poll at 64 raw frames and 256
//!   read attempts, preserving unread suffixes and distinguishing quiescence from incomplete work.
//! - [`rx_time`]: SocketCAN kernel receive times mapped onto `Instant`, never later than
//!   the read; absent or implausible stamps are counted and logged.
//! - [`state::MotorState`]: replaceable latest-state compatibility projection, not fault authority.
//! - [`wire`]: direction-aware classification of captured frames (host command vs drive
//!   frame) for offline candump analysis.
//! - Optional SocketCAN backend (`socketcan` feature, Linux).
//!   Received envelopes retain actual classic-CAN length and Data/Remote/Error class;
//!   only extended Data frames with exactly eight bytes become status or detailed faults.
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
//! | Davout | Sole production path through addressed `MotorBus` methods |
//! | Tests | [`MemoryBus`](bus::MemoryBus) without hardware |
//!
//! Wire spec: [hardware/docs/decisions/0002-robstride-protocol.md](../../hardware/docs/decisions/0002-robstride-protocol.md).

pub mod bus;
pub mod comm;
pub mod command;
pub mod feedback;
pub mod identity;
pub mod lifecycle;
pub mod mit;
pub mod motor_type;
pub mod params;
pub mod receive;
pub mod rx_time;
pub mod state;
pub mod version;
pub mod wire;

pub use bus::{
    AddressedMitCommand, BusError, CanBus, CanFrame, MemoryBus, MemoryRxQueue, MotorAddress,
    MotorBus, ReceivedCanFrame, RuntimeBus, RxFrameKind, TimedCanFrame,
};
#[cfg(all(feature = "socketcan", target_os = "linux"))]
pub use bus::{SocketCanBus, SocketCanRouter};
pub use comm::{pack_ext_id, unpack_ext_id, CommunicationType, ExtendedId, DEFAULT_HOST_ID};
pub use command::{CommandError, CommandField};
pub use feedback::{
    decode_status_header, is_reboot_transition, DetailedFaultFeedback, DriveMode, EchoedCommand,
    FeedbackEvent, FeedbackObservation, FeedbackReport, HostEchoObservation, IdentityObservation,
    MalformedFeedback, MalformedReason, ParameterReadObservation, TransportObservation,
};
pub use identity::{
    decode_device_id_reply, encode_default_get_device_id, encode_get_device_id, DeviceIdReply,
    DeviceUid, DEVICE_ID_REPLY_MARKER,
};
pub use lifecycle::{
    encode_active_reporting, encode_default_active_reporting, encode_default_disable,
    encode_default_enable, encode_default_fault_clear, encode_default_set_zero_position,
    encode_disable, encode_enable, encode_fault_clear, encode_set_zero_position,
};
pub use mit::{decode_mit_command_fields, encode_mit, MitCommand, MitFeedback};
pub use params::{
    decode_read_parameter_reply, encode_current_ref, encode_position_ref, encode_read_parameter,
    encode_set_run_mode, encode_speed_ref, encode_write_parameter, ParameterId, ParameterKind,
    ParameterReadReply, ParameterValue, RunMode,
};
pub use receive::{
    RawReceiveReport, ReceiveAttempt, ReceiveCompletion, ReceiveLimits, MAX_RX_ATTEMPTS_PER_POLL,
    MAX_RX_FRAMES_PER_POLL,
};
pub use rx_time::{RxTimestampCounts, MAX_KERNEL_RX_AGE};
pub use state::MotorState;
pub use version::{
    decode_firmware_version_reply, encode_default_get_firmware_version,
    encode_get_firmware_version, FirmwareVersion, FirmwareVersionFeedback, FIRMWARE_VERSION_QUERY,
};
pub use wire::{classify_frame, DriveFrame, HostCommand, MitCommandFields, WireFrame};

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
    use super::comm::{unpack_ext_id, CommunicationType};
    use super::mit::{encode_mit, MitCommand};
    use super::CanFrame;

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

        fn recv_one_nonblocking(&mut self) -> Result<super::ReceiveAttempt, super::BusError> {
            if self.rx.is_empty() {
                return Ok(super::ReceiveAttempt::Idle);
            }
            Ok(super::ReceiveAttempt::Frame(super::TimedCanFrame {
                received_at: std::time::Instant::now(),
                received: self.rx.remove(0),
            }))
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
        let (id, _) = encode_mit(&cmd).expect("valid command");
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
        bus.mit_control_all_at(&[AddressedMitCommand {
            address: MotorAddress::new("can0", 1),
            command: cmd,
        }])
        .expect("send");
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
    fn literal_identity_and_parameter_replies_are_addressed_but_never_poses() {
        let frame = |id: u32, data: [u8; 8]| CanFrame {
            id,
            data,
            extended: true,
        };
        let mut bus = MemoryBus::default();
        for item in [
            // Our own requests, as a loopback or another listener would see them.
            frame(0x0000_FD01, [0; 8]),
            frame(0x1100_FD01, [0x19, 0x70, 0, 0, 0, 0, 0, 0]),
            // Bench probe replies (firmware 0.3.1.42).
            frame(
                0x0000_01FE,
                [0x45, 0x7B, 0x30, 0x02, 0x0C, 0x32, 0x38, 0x17],
            ),
            frame(
                0x1100_01FD,
                [0x19, 0x70, 0x00, 0x00, 0x1D, 0xC7, 0x32, 0xB8],
            ),
            // Unconfigured device 9 and a reply addressed to another host.
            frame(0x0000_09FE, [1; 8]),
            frame(0x1100_01AA, [0x19, 0x70, 0, 0, 0, 0, 0, 0]),
        ] {
            bus.rx_queue.push(item);
        }
        let types = HashMap::from([(MotorAddress::new("can0", 1), MotorType::Rs03)]);
        let report = bus.recv_feedback_report(&types, Duration::ZERO, Duration::ZERO);
        assert!(report.completion.is_complete());
        assert!(report.terminal_error.is_none());
        assert!(
            report.observations.is_empty(),
            "replies are not status poses"
        );
        assert_eq!(report.identities.len(), 1);
        assert_eq!(report.identities[0].order, 2);
        assert_eq!(report.identities[0].address.device_id, 1);
        assert_eq!(
            report.identities[0].uid,
            super::DeviceUid([0x45, 0x7B, 0x30, 0x02, 0x0C, 0x32, 0x38, 0x17])
        );
        assert_eq!(report.parameter_reads.len(), 1);
        let read = &report.parameter_reads[0];
        assert_eq!(read.order, 3);
        assert_eq!(read.reply.parameter(), Some(super::ParameterId::MechPos));
        assert!((read.reply.as_f32() + 4.26e-5).abs() < 1e-6);
        // A positive-budget drain with only replies is not a benign timeout.
        let mut bus = MemoryBus::default();
        bus.rx_queue.push(frame(
            0x0000_01FE,
            [0x45, 0x7B, 0x30, 0x02, 0x0C, 0x32, 0x38, 0x17],
        ));
        let report = bus.recv_feedback_report(&types, Duration::from_millis(1), Duration::ZERO);
        assert!(report.terminal_error.is_none());
        assert_eq!(report.identities.len(), 1);
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
        let expected_id = encode_mit(&cmd).expect("valid command").0;
        let deadline = std::time::Instant::now() + Duration::from_millis(100);
        let mut frames: Vec<ReceivedCanFrame> = Vec::new();
        while std::time::Instant::now() < deadline {
            let report = router.recv_raw_report(
                Duration::ZERO,
                Duration::ZERO,
                super::ReceiveLimits::default(),
            );
            frames.extend(report.frames.into_iter().map(|timed| timed.received));
            assert!(report.terminal_error.is_none(), "recv routed vcan frames");
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
