#![allow(clippy::expect_used)]

use robstride::{unpack_ext_id, BusError, CanBus, CanFrame, MemoryBus, MotorBus, ReceiveAttempt};

use super::*;

const POSE: [u8; 8] = [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8];
const CAN_TIMEOUT_COUNTS: u32 = 600;

fn frame(id: u32, data: [u8; 8]) -> CanFrame {
    CanFrame {
        id,
        data,
        extended: true,
    }
}

fn firmware(device: u8) -> [u8; 4] {
    match device {
        1 | 2 => [0, 3, 1, 42],
        3 | 4 => [0, 2, 3, 34],
        _ => [0, 0, 3, 32],
    }
}

fn register(index: u16) -> [u8; 4] {
    match ParameterId::from_u16(index) {
        Some(ParameterId::CanTimeout) => CAN_TIMEOUT_COUNTS.to_le_bytes(),
        Some(ParameterId::MechPos) => 0.25_f32.to_le_bytes(),
        Some(ParameterId::ZeroSta) => [1, 0, 0, 0],
        _ => [0; 4],
    }
}

fn read_reply(device: u8, index: u16, value: [u8; 4]) -> CanFrame {
    let [lo, hi] = index.to_le_bytes();
    frame(
        0x1100_0000 | (u32::from(device) << 8) | u32::from(DEFAULT_HOST_ID),
        [lo, hi, 0, 0, value[0], value[1], value[2], value[3]],
    )
}

/// Five drives on a non-echoing [`MemoryBus`] that answer each request the way
/// the bench firmware does, plus scripted extra replies.
#[derive(Default)]
struct Drives {
    bus: MemoryBus,
    /// Status-identifier mode bits (22..23) on version replies.
    version_mode: u32,
    /// Host byte on version replies; 0 means [`DEFAULT_HOST_ID`].
    version_host: u8,
    /// After the reply to this exact request (id and data), also deliver the frame now.
    extra_now: Option<((u32, [u8; 8]), CanFrame)>,
    /// After the reply to this exact request, deliver the frame at the next write.
    extra_later: Option<((u32, [u8; 8]), CanFrame)>,
    later: Vec<CanFrame>,
}

impl Drives {
    fn reply(&self, request: &CanFrame) -> Option<CanFrame> {
        let ext = unpack_ext_id(request.id)?;
        let device = ext.device_id;
        let to_host = |kind: u32| kind | (u32::from(device) << 8) | u32::from(DEFAULT_HOST_ID);
        match ext.comm_type {
            0 => Some(frame(
                (u32::from(device) << 8) | 0xFE,
                [device, 0xA5, 0, 0, 0, 0, 0, 0x3C],
            )),
            4 if request.data[1] == 0xC4 => {
                let host = if self.version_host == 0 {
                    DEFAULT_HOST_ID
                } else {
                    self.version_host
                };
                let [a, b, c, d] = firmware(device);
                Some(frame(
                    0x0200_0000 | self.version_mode | (u32::from(device) << 8) | u32::from(host),
                    [0, 0xC4, 0x56, a, b, c, d, 0],
                ))
            }
            4 => Some(frame(to_host(0x0200_0000), POSE)),
            17 => {
                let index = u16::from_le_bytes([request.data[0], request.data[1]]);
                Some(read_reply(device, index, register(index)))
            }
            _ => None,
        }
    }
}

impl CanBus for Drives {
    fn send_frame(&mut self, request: &CanFrame) -> Result<(), BusError> {
        self.bus.send_frame(request)?;
        for frame in self.later.drain(..) {
            self.bus.rx_queue.push(frame);
        }
        if let Some(reply) = self.reply(request) {
            self.bus.rx_queue.push(reply);
        }
        let sent = (request.id, request.data);
        if let Some((_, extra)) = self.extra_now.as_ref().filter(|(on, _)| *on == sent) {
            self.bus.rx_queue.push(extra.clone());
        }
        if let Some((_, extra)) = self.extra_later.as_ref().filter(|(on, _)| *on == sent) {
            self.later.push(extra.clone());
        }
        Ok(())
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        self.bus.recv_one_nonblocking()
    }
}

impl MotorBus for Drives {}

fn owner(drives: Drives) -> Supervisor<Drives> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    Supervisor::from_repo_for_protocol_inspection(root, drives).expect("inspection owner")
}

/// Every frame a read-only inspection may write, built by the robstride encoders.
fn allowed_frames(owner: &Supervisor<Drives>) -> Vec<(u32, [u8; 8])> {
    let mut allowed = Vec::new();
    for motor in owner.stop_motors.iter() {
        let device = motor.device_id;
        allowed.push(robstride::encode_default_disable(device));
        allowed.push(robstride::encode_default_active_reporting(device, false));
        allowed.push(robstride::encode_default_get_firmware_version(device));
        allowed.push(robstride::encode_default_get_device_id(device));
        for parameter in INSPECTED_PARAMETERS {
            allowed.push(robstride::encode_read_parameter(
                DEFAULT_HOST_ID,
                device,
                parameter,
            ));
        }
    }
    allowed
}

fn assert_read_only_wire(owner: &Supervisor<Drives>) {
    let allowed = allowed_frames(owner);
    for sent in &owner.bus.bus.tx {
        assert!(
            sent.extended && allowed.contains(&(sent.id, sent.data)),
            "frame {:#010x} {:02x?} is not a stop, reporting Off or read query",
            sent.id,
            sent.data
        );
    }
}

/// The inspection always ends with a Disable to every installed address.
fn assert_ends_with_full_stop(owner: &Supervisor<Drives>) {
    let tx = &owner.bus.bus.tx;
    let addresses = owner.stop_motors.len();
    assert!(tx.len() >= addresses);
    for (sent, motor) in tx[tx.len() - addresses..]
        .iter()
        .zip(owner.stop_motors.iter())
    {
        assert_eq!(
            (sent.id, sent.data),
            robstride::encode_default_disable(motor.device_id)
        );
    }
}

#[test]
fn constructor_transmits_nothing() {
    let owner = owner(Drives::default());
    assert!(owner.bus.bus.tx.is_empty());
}

#[test]
fn inspects_every_drive_with_read_frames_only() {
    let mut owner = owner(Drives::default());
    let inspections = owner.inspect_drive_protocol(&[]).expect("inspection");

    assert_eq!(inspections.len(), owner.stop_motors.len());
    for inspection in &inspections {
        let device = inspection.address.device_id;
        assert_eq!(inspection.firmware, FirmwareVersion(firmware(device)));
        assert_eq!(inspection.uid.0[0], device);
        assert_eq!(
            inspection.parameter(ParameterId::CanTimeout),
            Some(ParameterValue::U32(CAN_TIMEOUT_COUNTS))
        );
        assert_eq!(
            inspection.parameter(ParameterId::MechPos),
            Some(ParameterValue::F32(0.25))
        );
        assert_eq!(
            inspection.parameter(ParameterId::ZeroSta),
            Some(ParameterValue::U8(1))
        );
        assert_eq!(inspection.parameters.len(), INSPECTED_PARAMETERS.len());
    }
    assert_read_only_wire(&owner);
    assert_ends_with_full_stop(&owner);
    assert_eq!(owner.mode(), OperationalMode::Disabled);
    assert!(!owner.has_latched_fault());
    assert!(
        owner.set_homing_complete().is_err(),
        "inspection never grants"
    );
}

#[test]
fn selected_joints_only_are_queried() {
    let mut owner = owner(Drives::default());
    let elbow = owner
        .stop_motors
        .iter()
        .find(|motor| motor.joint == "right_elbow_pitch")
        .expect("elbow")
        .device_id;
    let inspections = owner
        .inspect_drive_protocol(&["right_elbow_pitch".to_string()])
        .expect("inspection");
    assert_eq!(inspections.len(), 1);
    assert_eq!(inspections[0].address.device_id, elbow);
    let queried: Vec<u8> = owner
        .bus
        .bus
        .tx
        .iter()
        .filter(|sent| sent.id >> 24 == 17)
        .map(|sent| (sent.id & 0xFF) as u8)
        .collect();
    assert_eq!(queried, vec![elbow; INSPECTED_PARAMETERS.len()]);
}

#[test]
fn version_reply_outside_inspection_latches_and_is_never_pose() {
    let mut owner = owner(Drives::default());
    let [a, b, c, d] = firmware(1);
    owner
        .bus
        .bus
        .rx_queue
        .push(frame(0x0200_01FD, [0, 0xC4, 0x56, a, b, c, d, 0]));
    let error = owner.drain_feedback().expect_err("unsolicited version");
    assert!(
        error.to_string().contains("unsolicited firmware version"),
        "{error}"
    );
    assert!(owner.has_latched_fault());
    assert!(
        owner.motor_states.is_empty(),
        "version bytes never renew pose"
    );
}

#[test]
fn unknown_joint_is_refused_before_any_write() {
    let mut owner = owner(Drives::default());
    let error = owner
        .inspect_drive_protocol(&["no_such_joint".to_string()])
        .expect_err("unknown joint");
    assert!(matches!(error, DavoutError::UnknownJoint { .. }));
    assert!(owner.bus.bus.tx.is_empty());
}

fn can_timeout_request(device: u8) -> (u32, [u8; 8]) {
    robstride::encode_read_parameter(DEFAULT_HOST_ID, device, ParameterId::CanTimeout)
}

fn contradicting_can_timeout(device: u8) -> CanFrame {
    read_reply(
        device,
        ParameterId::CanTimeout.as_u16(),
        0_u32.to_le_bytes(),
    )
}

#[test]
fn contradictory_replies_in_one_drain_refuse_and_latch() {
    let mut owner = owner(Drives {
        extra_now: Some((can_timeout_request(3), contradicting_can_timeout(3))),
        ..Drives::default()
    });
    let error = owner
        .inspect_drive_protocol(&[])
        .expect_err("contradiction");
    assert!(error.to_string().contains("contradicts"), "{error}");
    assert!(owner.has_latched_fault());
    assert_read_only_wire(&owner);
    assert_ends_with_full_stop(&owner);
}

#[test]
fn contradiction_after_acceptance_refuses_and_latches() {
    let mut owner = owner(Drives {
        extra_later: Some((can_timeout_request(3), contradicting_can_timeout(3))),
        ..Drives::default()
    });
    let error = owner
        .inspect_drive_protocol(&[])
        .expect_err("late contradiction");
    assert!(
        error
            .to_string()
            .contains("contradicts the reply already accepted"),
        "{error}"
    );
    assert!(owner.has_latched_fault());
    assert_ends_with_full_stop(&owner);
}

#[test]
fn identical_duplicate_reply_is_not_a_contradiction() {
    let mut owner = owner(Drives {
        extra_later: Some((
            can_timeout_request(3),
            read_reply(
                3,
                ParameterId::CanTimeout.as_u16(),
                CAN_TIMEOUT_COUNTS.to_le_bytes(),
            ),
        )),
        ..Drives::default()
    });
    owner.inspect_drive_protocol(&[]).expect("duplicate agrees");
    assert!(!owner.has_latched_fault());
}

#[test]
fn reply_no_query_asked_for_refuses() {
    // A read of drive 2 arriving while drive 1 is queried: another host is asking.
    let mut owner = owner(Drives {
        extra_now: Some((
            can_timeout_request(1),
            read_reply(2, ParameterId::RunMode.as_u16(), [0; 4]),
        )),
        ..Drives::default()
    });
    let error = owner.inspect_drive_protocol(&[]).expect_err("unsolicited");
    assert!(error.to_string().contains("another CAN owner"), "{error}");
    assert!(owner.has_latched_fault());
    assert_ends_with_full_stop(&owner);
}

#[test]
fn version_reply_to_another_host_refuses() {
    let mut owner = owner(Drives {
        version_host: 0xA0,
        ..Drives::default()
    });
    let error = owner.inspect_drive_protocol(&[]).expect_err("foreign host");
    assert!(error.to_string().contains("another CAN owner"), "{error}");
    assert_ends_with_full_stop(&owner);
}

#[test]
fn drive_not_in_reset_refuses() {
    let mut owner = owner(Drives {
        version_mode: 2 << 22,
        ..Drives::default()
    });
    let error = owner.inspect_drive_protocol(&[]).expect_err("Run header");
    assert!(
        error.to_string().contains("unexpected drive mode Run"),
        "{error}"
    );
    assert!(owner.has_latched_fault());
    assert_read_only_wire(&owner);
    assert_ends_with_full_stop(&owner);
}
