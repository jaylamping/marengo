//! Direction-aware classification of one captured Robstride frame.
//!
//! A passive capture (candump) holds both this host's commands and the drives'
//! frames. The communication type alone does not say who sent a frame: types 0,
//! 17 and 24 travel both ways. Direction follows from where the host id sits:
//!
//! | Type | Host → drive | Drive → host |
//! |------|--------------|--------------|
//! | 0 | `host << 8 \| device` | `device << 8 \| 0xFE` |
//! | 1, 3, 4, 6, 18 | `extra << 8 \| device` (host-only types) | — |
//! | 2, 21 | — | `mode/flags << 16 \| device << 8 \| host` |
//! | 17 | `host << 8 \| device` | `status << 16 \| device << 8 \| host` |
//! | 24 | `host << 8 \| device` | `mode/flags << 16 \| device << 8 \| host` |
//!
//! This is evidence projection for offline analysis; the live receive path in
//! [`bus`](crate::bus) keeps its own address-checked admission.

use crate::comm::{unpack_ext_id, CommunicationType};
use crate::feedback::DriveMode;
use crate::identity::decode_device_id_reply;
use crate::mit::decode_mit_command_fields;
use crate::params::decode_read_parameter_reply;

/// Raw MIT command fields exactly as quantized on the wire (vendor u16 codes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MitCommandFields {
    pub position: u16,
    pub velocity: u16,
    pub kp: u16,
    pub kd: u16,
    /// Torque feedforward, carried in the identifier's `extra_data`.
    pub torque_ff: u16,
}

impl MitCommandFields {
    /// Vendor code of zero torque feedforward (`encode_mit` maps 0 Nm here).
    pub const NEUTRAL_TORQUE: u16 = 0x7FFF;

    /// Zero gains and torque feedforward within one quantization step of zero:
    /// the frame cannot produce torque whatever its position/velocity targets.
    pub fn is_neutral(self) -> bool {
        self.kp == 0 && self.kd == 0 && self.torque_ff.abs_diff(Self::NEUTRAL_TORQUE) <= 1
    }
}

/// A command this host (the `host_id` given to [`classify_frame`]) sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCommand {
    GetDeviceId,
    Mit(MitCommandFields),
    Enable,
    Disable,
    SetZero,
    ReadParameter {
        index: u16,
    },
    WriteParameter {
        index: u16,
    },
    /// Type 24 with `F_CMD` (byte 6) 1 = on, anything else = off.
    ActiveReporting {
        on: bool,
    },
}

/// A frame a drive sent to the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveFrame {
    /// Type 2: reply to a host command.
    Status { mode: DriveMode, flags: u8 },
    /// Type 24: periodic active report.
    Report { mode: DriveMode, flags: u8 },
    /// Type 21 detailed fault/warning frame.
    FaultReport,
    /// Type 0 reply (MCU UID).
    Identity,
    /// Type 17 reply.
    ParameterRead { index: u16, status: u8 },
}

impl DriveFrame {
    /// Drive mode for status-shaped frames (types 2 and 24).
    pub fn mode(self) -> Option<DriveMode> {
        match self {
            Self::Status { mode, .. } | Self::Report { mode, .. } => Some(mode),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireFrame {
    Host { device_id: u8, command: HostCommand },
    Drive { device_id: u8, frame: DriveFrame },
}

/// Classify a complete eight-byte extended frame relative to `host_id`.
/// Frames of another host, unknown types or short payloads return `None`.
pub fn classify_frame(host_id: u8, can_id: u32, data: &[u8]) -> Option<WireFrame> {
    let payload: &[u8; 8] = data.try_into().ok()?;
    let ext = unpack_ext_id(can_id)?;
    let comm = CommunicationType::from_u8(ext.comm_type)?;
    let extra_high = (ext.extra_data >> 8) as u8;
    let extra_low = (ext.extra_data & 0xFF) as u8;
    let status_header = || (DriveMode::from_can_id(can_id), extra_high & 0x3F);
    let from_host = extra_low == host_id && extra_high == 0;
    let host = |command| {
        Some(WireFrame::Host {
            device_id: ext.device_id,
            command,
        })
    };
    let drive = |device_id, frame| Some(WireFrame::Drive { device_id, frame });
    match comm {
        CommunicationType::GetDeviceId => match decode_device_id_reply(can_id, payload) {
            Some(reply) => drive(reply.device_id, DriveFrame::Identity),
            None if from_host => host(HostCommand::GetDeviceId),
            None => None,
        },
        CommunicationType::OperationControl => host(HostCommand::Mit(decode_mit_command_fields(
            can_id, payload,
        )?)),
        CommunicationType::Enable if from_host => host(HostCommand::Enable),
        CommunicationType::Disable if from_host => host(HostCommand::Disable),
        CommunicationType::SetZeroPosition if from_host => host(HostCommand::SetZero),
        CommunicationType::WriteParameter if from_host => host(HostCommand::WriteParameter {
            index: u16::from_le_bytes([payload[0], payload[1]]),
        }),
        CommunicationType::ReadParameter => {
            match decode_read_parameter_reply(host_id, can_id, payload) {
                Some(reply) => drive(
                    reply.device_id,
                    DriveFrame::ParameterRead {
                        index: reply.index,
                        status: reply.status,
                    },
                ),
                None if from_host => host(HostCommand::ReadParameter {
                    index: u16::from_le_bytes([payload[0], payload[1]]),
                }),
                None => None,
            }
        }
        CommunicationType::ActiveReporting if from_host => host(HostCommand::ActiveReporting {
            on: payload[6] == 0x01,
        }),
        CommunicationType::ActiveReporting if ext.device_id == host_id => {
            let (mode, flags) = status_header();
            drive(extra_low, DriveFrame::Report { mode, flags })
        }
        CommunicationType::OperationStatus if ext.device_id == host_id => {
            let (mode, flags) = status_header();
            drive(extra_low, DriveFrame::Status { mode, flags })
        }
        CommunicationType::FaultReport if ext.device_id == host_id => {
            drive(extra_low, DriveFrame::FaultReport)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comm::DEFAULT_HOST_ID;

    fn classify(id: u32, data: [u8; 8]) -> Option<WireFrame> {
        classify_frame(DEFAULT_HOST_ID, id, &data)
    }

    // Literal frames from bench candump cd-20261003T145133Z.log (firmware 0.3.1.42).
    #[test]
    fn host_commands_carry_the_host_id_above_the_target() {
        assert_eq!(
            classify(0x0300_FD01, [0; 8]),
            Some(WireFrame::Host {
                device_id: 1,
                command: HostCommand::Enable
            })
        );
        assert_eq!(
            classify(0x0600_FD01, [1, 0, 0, 0, 0, 0, 0, 0]),
            Some(WireFrame::Host {
                device_id: 1,
                command: HostCommand::SetZero
            })
        );
        assert_eq!(
            classify(0x1800_FD01, [1, 2, 3, 4, 5, 6, 0, 0]),
            Some(WireFrame::Host {
                device_id: 1,
                command: HostCommand::ActiveReporting { on: false }
            })
        );
        assert_eq!(
            classify(0x1100_FD01, [0x19, 0x70, 0, 0, 0, 0, 0, 0]),
            Some(WireFrame::Host {
                device_id: 1,
                command: HostCommand::ReadParameter { index: 0x7019 }
            })
        );
        assert_eq!(
            classify(0x0000_FD02, [0; 8]),
            Some(WireFrame::Host {
                device_id: 2,
                command: HostCommand::GetDeviceId
            })
        );
    }

    #[test]
    fn drive_frames_carry_the_device_above_the_host() {
        assert_eq!(
            classify(
                0x0280_01FD,
                [0x7F, 0xFE, 0x7F, 0xEC, 0x7F, 0xA0, 0x00, 0xE6]
            ),
            Some(WireFrame::Drive {
                device_id: 1,
                frame: DriveFrame::Status {
                    mode: DriveMode::Run,
                    flags: 0
                }
            })
        );
        assert_eq!(
            classify(
                0x1800_01FD,
                [0x7F, 0xFF, 0x7F, 0x6F, 0x7F, 0xFF, 0x00, 0xE6]
            ),
            Some(WireFrame::Drive {
                device_id: 1,
                frame: DriveFrame::Report {
                    mode: DriveMode::Reset,
                    flags: 0
                }
            })
        );
        assert_eq!(
            classify(
                0x0000_01FE,
                [0x45, 0x7B, 0x30, 0x02, 0x0C, 0x32, 0x38, 0x17]
            ),
            Some(WireFrame::Drive {
                device_id: 1,
                frame: DriveFrame::Identity
            })
        );
        assert_eq!(
            classify(0x1100_01FD, [0x19, 0x70, 0, 0, 0x1D, 0x07, 0x06, 0x39]),
            Some(WireFrame::Drive {
                device_id: 1,
                frame: DriveFrame::ParameterRead {
                    index: 0x7019,
                    status: 0
                }
            })
        );
    }

    #[test]
    fn mit_neutrality_is_judged_on_raw_codes() {
        let fields = MitCommandFields {
            position: 0x7FFF,
            velocity: 0x7FFF,
            kp: 0,
            kd: 0,
            torque_ff: 0x7FFF,
        };
        assert_eq!(
            classify(0x017F_FF01, [0x7F, 0xFF, 0x7F, 0xFF, 0, 0, 0, 0]),
            Some(WireFrame::Host {
                device_id: 1,
                command: HostCommand::Mit(fields),
            })
        );
        assert!(fields.is_neutral());
        assert!(MitCommandFields {
            torque_ff: 0x8000,
            ..fields
        }
        .is_neutral());
        assert!(!MitCommandFields {
            torque_ff: 0x8001,
            ..fields
        }
        .is_neutral());
        assert!(!MitCommandFields { kd: 1, ..fields }.is_neutral());
    }

    #[test]
    fn other_hosts_and_short_payloads_are_not_classified() {
        assert_eq!(classify_frame(DEFAULT_HOST_ID, 0x0300_FE01, &[0; 8]), None);
        assert_eq!(classify_frame(DEFAULT_HOST_ID, 0x0300_FD01, &[0; 4]), None);
    }
}
