//! Test-only Robstride firmware emulator behind the public `MotorBus` seam.
//!
//! Each configured address owns an encoder, a volatile zero offset, enable and
//! reporting flags, an MCU identifier and fault knobs. Replies are queued at
//! transmit time in wire order; type-24 reports are emitted only when the test
//! pumps. Inbound status/reply identifiers follow the documented wire layout
//! (`type << 24 | extra << 8 | low`); outbound frames are produced by the
//! supervisor through the robstride encoders.
#![allow(dead_code, clippy::expect_used, clippy::panic)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

use marengo_config::{MotorEntry, MotorType};
use robstride::motor_type::MitRanges;
use robstride::{
    pack_ext_id, BusError, CanBus, CanFrame, CommunicationType, MotorBus, ParameterId,
    ReceiveAttempt, ReceivedCanFrame, TimedCanFrame, DEFAULT_HOST_ID,
};

const RESET_MODE: u32 = 0;
const RUN_MODE: u32 = 2;

/// One emulated drive at one configured address.
#[derive(Debug, Clone)]
pub struct Drive {
    pub joint: String,
    pub device_id: u8,
    pub motor_type: MotorType,
    /// `direction * gear_ratio`: motor = joint * scale.
    pub scale: f64,
    pub uid: [u8; 8],
    /// Absolute encoder, motor space.
    pub raw_motor_rad: f64,
    /// Volatile firmware zero, motor space; reported position is raw - offset.
    pub zero_offset_motor_rad: f64,
    pub enabled: bool,
    pub reporting: bool,
    /// A rebooting drive receives and answers nothing until this instant.
    pub silent_until: Option<Instant>,
    // Fault knobs.
    pub answer_identity: bool,
    pub drop_set_zero_ack: bool,
    /// Hold the Enable status reply and deliver it only after SetZero (stale ack).
    pub hold_enable_reply_until_set_zero: bool,
    held_reply: Option<CanFrame>,
    /// Added to every mechPos readback, joint space.
    pub readback_offset_joint_rad: f64,
    pub readback_status: u8,
    /// Reboot (lose zero, Reset, reporting off) right after queuing the SetZero
    /// ack, then stay silent for the given duration.
    pub reboot_after_ack: Option<Duration>,
    /// Communication types whose transmissions to this address fail.
    pub fail_writes: Vec<u8>,
    /// Communication types that start failing once a mechPos readback was answered.
    pub fail_writes_after_readback: Vec<u8>,
}

impl Drive {
    pub fn position_motor_rad(&self) -> f64 {
        self.raw_motor_rad - self.zero_offset_motor_rad
    }

    pub fn position_joint_rad(&self) -> f64 {
        self.position_motor_rad() / self.scale
    }

    fn mode(&self) -> u32 {
        if self.enabled {
            RUN_MODE
        } else {
            RESET_MODE
        }
    }

    fn responsive(&self, now: Instant) -> bool {
        self.silent_until.is_none_or(|until| now >= until)
    }

    /// Type-2 (or type-24) status in the drive's current mode.
    fn status(&self, comm_type: CommunicationType) -> CanFrame {
        let ranges = MitRanges::for_motor_type(self.motor_type);
        let position_scale = f64::from(ranges.position_scale);
        let position = ((self.position_motor_rad() / position_scale + 1.0) * 32767.0)
            .round()
            .clamp(0.0, f64::from(u16::MAX)) as u16;
        let mut data = [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8];
        data[0..2].copy_from_slice(&position.to_be_bytes());
        CanFrame {
            id: (u32::from(comm_type.as_u8()) << 24)
                | (self.mode() << 22)
                | (u32::from(self.device_id) << 8)
                | u32::from(DEFAULT_HOST_ID),
            data,
            extended: true,
        }
    }

    /// Type-0 reply: `(0 << 24) | (device << 8) | 0xFE`, payload = MCU UID.
    fn identity_reply(&self) -> CanFrame {
        CanFrame {
            id: pack_ext_id(
                CommunicationType::GetDeviceId.as_u8(),
                u16::from(self.device_id),
                robstride::identity::DEVICE_ID_REPLY_MARKER,
            ),
            data: self.uid,
            extended: true,
        }
    }

    /// Type-17 reply: `(17 << 24) | (status << 16) | (device << 8) | host`.
    fn read_reply(&self, index: u16, status: u8, value: [u8; 4]) -> CanFrame {
        let mut data = [0u8; 8];
        data[0..2].copy_from_slice(&index.to_le_bytes());
        data[4..8].copy_from_slice(&value);
        CanFrame {
            id: pack_ext_id(
                CommunicationType::ReadParameter.as_u8(),
                (u16::from(status) << 8) | u16::from(self.device_id),
                DEFAULT_HOST_ID,
            ),
            data,
            extended: true,
        }
    }

    /// Power cycle: the volatile zero and enable/reporting state are lost.
    pub fn reboot(&mut self, silent_until: Option<Instant>) {
        self.zero_offset_motor_rad = 0.0;
        self.enabled = false;
        self.reporting = false;
        self.held_reply = None;
        self.silent_until = silent_until;
    }
}

/// Every emulated drive plus the shared wire queues.
#[derive(Debug, Default)]
pub struct Firmware {
    pub drives: Vec<Drive>,
    rx: VecDeque<CanFrame>,
    /// Frames the transport accepted, in order.
    pub tx: Vec<CanFrame>,
    /// Frames whose transmission was refused by an injected failure.
    pub failed_tx: Vec<CanFrame>,
}

pub type SharedFirmware = Rc<RefCell<Firmware>>;

impl Firmware {
    /// One drive per configured motor at `initial[joint]` joint radians (0 if absent).
    pub fn from_motors(motors: &[MotorEntry], initial: &[(&str, f64)]) -> SharedFirmware {
        let drives = motors
            .iter()
            .map(|motor| {
                let scale = f64::from(motor.direction) * motor.gear_ratio;
                let joint = initial
                    .iter()
                    .find(|(name, _)| *name == motor.joint)
                    .map_or(0.0, |(_, position)| *position);
                Drive {
                    joint: motor.joint.clone(),
                    device_id: motor.device_id,
                    motor_type: motor.motor_type,
                    scale,
                    uid: [
                        0x45,
                        0x7b,
                        0x30,
                        0x02,
                        0x0c,
                        0x32,
                        0x38,
                        0x10 | motor.device_id,
                    ],
                    raw_motor_rad: joint * scale,
                    zero_offset_motor_rad: 0.0,
                    enabled: false,
                    reporting: false,
                    silent_until: None,
                    answer_identity: true,
                    drop_set_zero_ack: false,
                    hold_enable_reply_until_set_zero: false,
                    held_reply: None,
                    readback_offset_joint_rad: 0.0,
                    readback_status: 0,
                    reboot_after_ack: None,
                    fail_writes: Vec::new(),
                    fail_writes_after_readback: Vec::new(),
                }
            })
            .collect();
        Rc::new(RefCell::new(Self {
            drives,
            ..Self::default()
        }))
    }

    pub fn drive(&self, joint: &str) -> &Drive {
        self.drives
            .iter()
            .find(|drive| drive.joint == joint)
            .expect("emulated joint")
    }

    pub fn drive_mut(&mut self, joint: &str) -> &mut Drive {
        self.drives
            .iter_mut()
            .find(|drive| drive.joint == joint)
            .expect("emulated joint")
    }

    /// Periodic type-24 reports from every live drive whose reporting is on.
    pub fn emit_reports(&mut self) {
        let now = Instant::now();
        let frames: Vec<_> = self
            .drives
            .iter()
            .filter(|drive| drive.reporting && drive.responsive(now))
            .map(|drive| drive.status(CommunicationType::ActiveReporting))
            .collect();
        self.rx.extend(frames);
    }

    /// Accepted frames of `comm_type` addressed to `device_id` (outbound low byte).
    pub fn sent(&self, comm_type: CommunicationType, device_id: u8) -> usize {
        self.tx
            .iter()
            .filter(|frame| is_outbound(frame, comm_type, Some(device_id)))
            .count()
    }

    /// Accepted frames of `comm_type` to any address.
    pub fn sent_any(&self, comm_type: CommunicationType) -> usize {
        self.tx
            .iter()
            .filter(|frame| is_outbound(frame, comm_type, None))
            .count()
    }

    pub fn clear_trace(&mut self) {
        self.tx.clear();
        self.failed_tx.clear();
    }

    fn transmit(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        let comm_type = ((frame.id >> 24) & 0x1f) as u8;
        let device_id = (frame.id & 0xff) as u8;
        let now = Instant::now();
        let Some(index) = self
            .drives
            .iter()
            .position(|drive| drive.device_id == device_id)
        else {
            self.tx.push(frame.clone());
            return Ok(());
        };
        if self.drives[index].fail_writes.contains(&comm_type) {
            self.failed_tx.push(frame.clone());
            return Err(BusError::Send {
                message: format!("injected write failure to device {device_id}"),
            });
        }
        self.tx.push(frame.clone());
        let drive = &mut self.drives[index];
        if !drive.responsive(now) {
            return Ok(());
        }
        drive.silent_until = None;
        let mut replies = Vec::new();
        match CommunicationType::from_u8(comm_type) {
            Some(CommunicationType::GetDeviceId) => {
                if drive.answer_identity {
                    replies.push(drive.identity_reply());
                }
            }
            Some(CommunicationType::OperationControl) => {
                replies.push(drive.status(CommunicationType::OperationStatus));
            }
            Some(CommunicationType::Enable) => {
                drive.enabled = true;
                let reply = drive.status(CommunicationType::OperationStatus);
                if drive.hold_enable_reply_until_set_zero {
                    drive.held_reply = Some(reply);
                } else {
                    replies.push(reply);
                }
            }
            Some(CommunicationType::Disable) => {
                drive.enabled = false;
                replies.push(drive.status(CommunicationType::OperationStatus));
            }
            Some(CommunicationType::SetZeroPosition) => {
                drive.zero_offset_motor_rad = drive.raw_motor_rad;
                if let Some(stale) = drive.held_reply.take() {
                    replies.push(stale);
                }
                if !drive.drop_set_zero_ack {
                    replies.push(drive.status(CommunicationType::OperationStatus));
                }
                if let Some(silence) = drive.reboot_after_ack {
                    drive.reboot(Some(now + silence));
                }
            }
            Some(CommunicationType::ReadParameter) => {
                let index = u16::from_le_bytes([frame.data[0], frame.data[1]]);
                if index == ParameterId::MechPos.as_u16() {
                    let joint = drive.position_joint_rad() + drive.readback_offset_joint_rad;
                    let value = ((joint * drive.scale) as f32).to_le_bytes();
                    replies.push(drive.read_reply(index, drive.readback_status, value));
                    let armed = std::mem::take(&mut drive.fail_writes_after_readback);
                    drive.fail_writes.extend(armed);
                } else if index == ParameterId::RunMode.as_u16() {
                    replies.push(drive.read_reply(index, 0, [0; 4]));
                } else {
                    replies.push(drive.read_reply(index, 1, [0; 4]));
                }
            }
            Some(CommunicationType::ActiveReporting) => {
                drive.reporting = frame.data[6] == 0x01;
            }
            _ => {}
        }
        self.rx.extend(replies);
        Ok(())
    }
}

fn is_outbound(frame: &CanFrame, comm_type: CommunicationType, device_id: Option<u8>) -> bool {
    (frame.id >> 24) & 0x1f == u32::from(comm_type.as_u8())
        && ((frame.id >> 8) & 0xff) as u8 == DEFAULT_HOST_ID
        && device_id.is_none_or(|device| (frame.id & 0xff) as u8 == device)
}

/// Supervisor-owned transport handle; the test keeps the other `Rc`.
pub struct FirmwareBus(pub SharedFirmware);

impl CanBus for FirmwareBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.0.borrow_mut().transmit(frame)
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        Ok(match self.0.borrow_mut().rx.pop_front() {
            Some(frame) => ReceiveAttempt::Frame(TimedCanFrame {
                received_at: Instant::now(),
                received: ReceivedCanFrame::full_data(None, frame),
            }),
            None => ReceiveAttempt::Idle,
        })
    }
}

impl MotorBus for FirmwareBus {}
