//! Blocking standalone bench inspection. No reference selection or motion operations.

use std::time::{Duration, Instant};

use robstride::protocol::{
    encode_device_identity_query, encode_firmware_version_query, ProtocolObservation, ProtocolReply,
};
use robstride::{CanFrame, MotorAddress, MotorBus, ParameterId};
use serde::Serialize;

use super::{DavoutError, OperationalMode, Supervisor};

const QUERY_TIMEOUT: Duration = Duration::from_millis(300);
const MAX_INSPECTION_MOTORS: usize = 16;

#[derive(Debug, Clone, Serialize)]
pub struct ProtocolReadReceipt {
    pub query: String,
    pub request_id: u32,
    pub request_data: [u8; 8],
    pub response_id: u32,
    pub response_data: [u8; 8],
    pub response_host: u8,
    pub elapsed_us: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct MotorProtocolInspection {
    pub joint: String,
    pub interface: String,
    pub device_id: u8,
    pub identity_wire_bytes: [u8; 8],
    pub firmware_version: [u8; 4],
    pub reads: Vec<ProtocolReadReceipt>,
}

#[derive(Clone, Copy)]
pub(super) enum Query {
    Identity,
    Version,
    SetZero,
    Parameter(ParameterId),
}

impl Query {
    fn frame(self, host: u8, motor: u8) -> CanFrame {
        match self {
            Self::Identity => encode_device_identity_query(host, motor),
            Self::Version => encode_firmware_version_query(host, motor),
            Self::SetZero => {
                let (id, data) = robstride::encode_set_zero_position(host, motor);
                CanFrame {
                    id,
                    data,
                    extended: true,
                }
            }
            Self::Parameter(parameter) => {
                let (id, data) = robstride::encode_read_parameter(host, motor, parameter);
                CanFrame {
                    id,
                    data,
                    extended: true,
                }
            }
        }
    }

    fn accepts(self, observation: &ProtocolObservation, host: u8) -> bool {
        match (self, observation.reply) {
            (Self::Identity, ProtocolReply::DeviceIdentity(_)) => observation.host_id == 0xFE,
            (Self::Version, ProtocolReply::FirmwareVersion(_)) => observation.host_id == host,
            (Self::SetZero, ProtocolReply::StatusHeader { .. }) => observation.host_id == host,
            (Self::Parameter(parameter), ProtocolReply::Parameter { index, .. }) => {
                observation.host_id == host && index == parameter.as_u16()
            }
            _ => false,
        }
    }

    fn label(self) -> String {
        match self {
            Self::Identity => "identity".into(),
            Self::Version => "firmware_version".into(),
            Self::SetZero => "set_zero".into(),
            Self::Parameter(parameter) => format!("{parameter:?}"),
        }
    }
}

impl<B: MotorBus> Supervisor<B> {
    /// Inspect actual replies while Disabled, then stop all installed addresses again.
    ///
    /// Blocking, with at most sixteen motors and eight 300 ms query deadlines each.
    /// Intended for a standalone CLI with exclusive CAN ownership, never a realtime tick.
    /// This operation cannot create current-reference permission, Ready or Active.
    pub fn inspect_drive_protocol(&mut self) -> Result<Vec<MotorProtocolInspection>, DavoutError> {
        self.refuse_reference_interference("drive protocol inspection")?;
        self.require_fault_clear()?;
        if self.mode != OperationalMode::Disabled || self.hardware_estop {
            return Err(DavoutError::ProtocolInspection {
                joint: String::new(),
                message: "requires Disabled and clear E-stop".into(),
            });
        }
        if self.motors.motors.is_empty() || self.motors.motors.len() > MAX_INSPECTION_MOTORS {
            return Err(DavoutError::ProtocolInspection {
                joint: String::new(),
                message: "inspection supports one to sixteen motors".into(),
            });
        }
        let result = (|| {
            self.inspection_stop()?;
            // Explicitly turn streams off; never restore/reassert On in this lifecycle.
            let mut reporting_error = None;
            for motor in self.stop_motors.clone() {
                if let Err(error) = self
                    .bus
                    .disable_active_reporting_at(&MotorAddress::from(&motor))
                {
                    let error = DavoutError::Bus(error);
                    self.record_runtime_error(&error);
                    reporting_error.get_or_insert(error);
                }
            }
            if let Some(error) = reporting_error {
                return Err(error);
            }
            self.inspect_drive_protocol_inner()
        })();
        if let Err(error) = &result {
            self.record_runtime_error(error);
        }
        let cleanup = self.inspection_stop();
        match (result, cleanup) {
            (Ok(receipts), Ok(())) => Ok(receipts),
            (Err(error), Ok(())) => Err(error),
            (result, Err(cleanup)) => Err(DavoutError::ProtocolInspection {
                joint: String::new(),
                message: format!(
                    "inspection result: {:?}; final stop failed: {cleanup}",
                    result.err()
                ),
            }),
        }
    }

    pub(super) fn inspection_stop(&mut self) -> Result<(), DavoutError> {
        let failed_writes = self.perform_stop(false).failed_writes();
        if failed_writes > 0 {
            Err(DavoutError::StopDelivery { failed_writes })
        } else {
            Ok(())
        }
    }

    pub(super) fn inspection_receive(
        &mut self,
        accepted: &[ProtocolObservation],
    ) -> Result<Vec<ProtocolObservation>, DavoutError> {
        let report =
            self.bus
                .recv_feedback_report(&self.motor_types, Duration::ZERO, Duration::ZERO);
        let replies = report.protocol_observations.clone();
        let consumption = self.consume_report_in_context(
            report,
            &super::feedback_consumer::ReceiveContext::DisabledInspection,
        );
        if let Some(error) = consumption.first_error {
            return Err(error);
        }
        self.require_fault_clear()?;
        for reply in &replies {
            for prior in accepted {
                let same_query = reply.address == prior.address
                    && reply.host_id == prior.host_id
                    && match (reply.reply, prior.reply) {
                        (ProtocolReply::DeviceIdentity(_), ProtocolReply::DeviceIdentity(_))
                        | (ProtocolReply::FirmwareVersion(_), ProtocolReply::FirmwareVersion(_)) => {
                            true
                        }
                        (
                            ProtocolReply::StatusHeader { .. },
                            ProtocolReply::StatusHeader { .. },
                        ) => true,
                        (
                            ProtocolReply::Parameter { index: a, .. },
                            ProtocolReply::Parameter { index: b, .. },
                        ) => a == b,
                        _ => false,
                    };
                if same_query && (reply.can_id != prior.can_id || reply.raw != prior.raw) {
                    return Err(DavoutError::ProtocolInspection {
                        joint: String::new(),
                        message: "conflicting reply to a completed query".into(),
                    });
                }
            }
        }
        Ok(replies)
    }

    fn inspect_drive_protocol_inner(
        &mut self,
    ) -> Result<Vec<MotorProtocolInspection>, DavoutError> {
        let motors = self.motors.motors.clone();
        let mut receipts = Vec::with_capacity(motors.len());
        // Never reuse a query host within this finite inspection. UID replies have
        // a vendor-fixed FE destination and do not echo this discriminator.
        let mut host = 0x80_u16;
        let mut accepted = Vec::with_capacity(motors.len() * 8);
        for motor in motors {
            let address = MotorAddress::from(&motor);
            let mut inspection = MotorProtocolInspection {
                joint: motor.joint.clone(),
                interface: motor.can_interface,
                device_id: motor.device_id,
                identity_wire_bytes: [0; 8],
                firmware_version: [0; 4],
                reads: Vec::new(),
            };
            let queries = [
                Query::Version,
                Query::Identity,
                Query::Parameter(ParameterId::RunMode),
                Query::Parameter(ParameterId::MechanicalPosition),
                Query::Parameter(ParameterId::MechanicalVelocity),
                Query::Parameter(ParameterId::CanTimeout),
                Query::Parameter(ParameterId::ZeroWrapping),
                Query::Parameter(ParameterId::AddOffset),
            ];
            for query in queries {
                let (reply, receipt) = self.inspect_protocol_query(
                    &motor.joint,
                    &address,
                    query,
                    host as u8,
                    &accepted,
                )?;
                host += 1;
                match reply.reply {
                    ProtocolReply::FirmwareVersion(version) => {
                        inspection.firmware_version = version
                    }
                    ProtocolReply::DeviceIdentity(identity) => {
                        inspection.identity_wire_bytes = identity
                    }
                    ProtocolReply::Parameter { .. } | ProtocolReply::StatusHeader { .. } => {}
                }
                inspection.reads.push(receipt);
                accepted.push(reply);
            }
            receipts.push(inspection);
        }
        self.inspection_receive(&accepted)?;
        Ok(receipts)
    }

    pub(super) fn inspect_protocol_query(
        &mut self,
        joint: &str,
        address: &MotorAddress,
        query: Query,
        host: u8,
        accepted: &[ProtocolObservation],
    ) -> Result<(ProtocolObservation, ProtocolReadReceipt), DavoutError> {
        // Consume stale queued hazards before issuing the diagnostic request.
        self.inspection_receive(accepted)?;
        let request = query.frame(host, address.device_id);
        let issued = Instant::now();
        self.bus.send_frame_to(address, &request)?;
        let deadline = issued + QUERY_TIMEOUT;
        loop {
            let replies = self.inspection_receive(accepted)?;
            let candidates: Vec<_> = replies
                .iter()
                .filter(|observation| {
                    observation.address == *address
                        && observation.received_at >= issued
                        && observation.received_at < deadline
                        && query.accepts(observation, host)
                })
                .cloned()
                .collect();
            if Instant::now() >= deadline {
                return Err(DavoutError::ProtocolInspection {
                    joint: joint.into(),
                    message: format!("{} timed out", query.label()),
                });
            }
            if let Some(reply) = candidates.first() {
                if candidates
                    .iter()
                    .any(|other| other.can_id != reply.can_id || other.raw != reply.raw)
                {
                    return Err(DavoutError::ProtocolInspection {
                        joint: joint.into(),
                        message: format!("conflicting {} replies", query.label()),
                    });
                }
                if let ProtocolReply::Parameter { error, .. } = reply.reply {
                    if error != 0 {
                        return Err(DavoutError::ProtocolInspection {
                            joint: joint.into(),
                            message: format!("{} returned error {error}", query.label()),
                        });
                    }
                }
                if matches!(query, Query::Version | Query::SetZero)
                    && ((reply.can_id >> 22) & 3) != 0
                {
                    return Err(DavoutError::ProtocolInspection {
                        joint: joint.into(),
                        message: "reply did not confirm Reset/Disabled".into(),
                    });
                }
                let receipt = ProtocolReadReceipt {
                    query: query.label(),
                    request_id: request.id,
                    request_data: request.data,
                    response_id: reply.can_id,
                    response_data: reply.raw,
                    response_host: reply.host_id,
                    elapsed_us: reply.received_at.duration_since(issued).as_micros(),
                };
                return Ok((reply.clone(), receipt));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use robstride::{CanBus, ReceiveAttempt, ReceivedCanFrame, TimedCanFrame};
    use std::collections::{HashMap, VecDeque};
    use std::path::PathBuf;

    #[derive(Default)]
    struct ProbeBus {
        tx: Vec<CanFrame>,
        rx: VecDeque<TimedCanFrame>,
        inject_peer_fault: bool,
        wrong_host: bool,
        fail_initial_stop: bool,
        fail_query: bool,
        conflicting_mode: bool,
        late_conflict: bool,
        deferred: Option<TimedCanFrame>,
        deferred_armed: bool,
        timeout_by_device: HashMap<u8, u32>,
        ignore_timeout_write: bool,
        bad_zero: bool,
        inject_run_before_zero: bool,
    }

    impl CanBus for ProbeBus {
        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), robstride::BusError> {
            self.tx.push(frame.clone());
            if self.fail_initial_stop && (frame.id >> 24) & 0x1F == 18 {
                self.fail_initial_stop = false;
                return Err(robstride::BusError::Send {
                    message: "initial stop fixture".into(),
                });
            }
            if self.fail_query && frame.data[1] == 0xC4 {
                return Err(robstride::BusError::Send {
                    message: "query fixture".into(),
                });
            }
            let id = robstride::unpack_ext_id(frame.id).expect("query ID");
            if id.comm_type == 18
                && frame.data[..2] == ParameterId::CanTimeout.as_u16().to_le_bytes()
                && !self.ignore_timeout_write
            {
                self.timeout_by_device.insert(
                    id.device_id,
                    u32::from_le_bytes([
                        frame.data[4],
                        frame.data[5],
                        frame.data[6],
                        frame.data[7],
                    ]),
                );
            }
            let host = if self.wrong_host {
                0xFF
            } else {
                id.extra_data as u8
            };
            let response = match id.comm_type {
                0 => Some(CanFrame {
                    id: robstride::pack_ext_id(0, u16::from(id.device_id), 0xFE),
                    data: [id.device_id; 8],
                    extended: true,
                }),
                4 if frame.data[1] == 0xC4 => Some(CanFrame {
                    id: robstride::pack_ext_id(2, u16::from(id.device_id), host),
                    data: match id.device_id {
                        1 | 2 => [0, 0xC4, 0x56, 0, 3, 1, 42, 0],
                        3 | 4 => [0, 0xC4, 0x56, 0, 2, 3, 34, 0],
                        _ => [0, 0xC4, 0x56, 0, 0, 3, 32, 0],
                    },
                    extended: true,
                }),
                6 => Some(CanFrame {
                    id: robstride::pack_ext_id(2, u16::from(id.device_id), host),
                    data: [0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0, 0xC8],
                    extended: true,
                }),
                17 => Some(CanFrame {
                    id: robstride::pack_ext_id(17, u16::from(id.device_id), host),
                    data: {
                        let index = u16::from_le_bytes([frame.data[0], frame.data[1]]);
                        let value = if index == ParameterId::CanTimeout.as_u16() {
                            self.timeout_by_device
                                .get(&id.device_id)
                                .copied()
                                .unwrap_or(0)
                                .to_le_bytes()
                        } else if index == ParameterId::MechanicalPosition.as_u16() && self.bad_zero
                        {
                            1.0_f32.to_le_bytes()
                        } else {
                            [0; 4]
                        };
                        [
                            frame.data[0],
                            frame.data[1],
                            0,
                            0,
                            value[0],
                            value[1],
                            value[2],
                            value[3],
                        ]
                    },
                    extended: true,
                }),
                _ => None,
            };
            if let Some(response) = response {
                let is_version = frame.data[1] == 0xC4;
                let mut conflict = response.clone();
                conflict.id |= 2 << 22;
                self.rx.push_back(TimedCanFrame {
                    received_at: Instant::now(),
                    received: ReceivedCanFrame::full_data(Some("can0".into()), response),
                });
                if is_version && self.conflicting_mode {
                    self.rx.push_back(TimedCanFrame {
                        received_at: Instant::now(),
                        received: ReceivedCanFrame::full_data(
                            Some("can0".into()),
                            conflict.clone(),
                        ),
                    });
                }
                if is_version && self.late_conflict {
                    self.late_conflict = false;
                    self.deferred = Some(TimedCanFrame {
                        received_at: Instant::now(),
                        received: ReceivedCanFrame::full_data(Some("can0".into()), conflict),
                    });
                }
                if self.inject_run_before_zero && id.comm_type == 17 && host == 0xB4 {
                    self.inject_run_before_zero = false;
                    self.deferred = Some(TimedCanFrame {
                        received_at: Instant::now(),
                        received: ReceivedCanFrame::full_data(
                            Some("can0".into()),
                            CanFrame {
                                id: 0x0280_01FD,
                                data: [0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0, 0xC8],
                                extended: true,
                            },
                        ),
                    });
                }
                if self.inject_peer_fault {
                    self.inject_peer_fault = false;
                    self.rx.push_back(TimedCanFrame {
                        received_at: Instant::now(),
                        received: ReceivedCanFrame::full_data(
                            Some("can0".into()),
                            CanFrame {
                                id: 0x0201_02FD,
                                data: [0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0, 0xC8],
                                extended: true,
                            },
                        ),
                    });
                }
            }
            Ok(())
        }
        fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, robstride::BusError> {
            if self.rx.is_empty() && self.deferred.is_some() {
                if self.deferred_armed {
                    return Ok(self
                        .deferred
                        .take()
                        .map(ReceiveAttempt::Frame)
                        .unwrap_or(ReceiveAttempt::Idle));
                }
                self.deferred_armed = true;
                return Ok(ReceiveAttempt::Idle);
            }
            Ok(self
                .rx
                .pop_front()
                .map(ReceiveAttempt::Frame)
                .unwrap_or(ReceiveAttempt::Idle))
        }
    }
    impl MotorBus for ProbeBus {}

    fn supervisor(bus: ProbeBus) -> Supervisor<ProbeBus> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        Supervisor::from_repo(&root, bus).expect("supervisor")
    }

    #[test]
    fn inspects_all_five_without_grant_or_enable() {
        let mut owner = supervisor(ProbeBus::default());
        let receipts = owner.inspect_drive_protocol().expect("inspection");
        assert_eq!(receipts.len(), 5);
        for receipt in receipts {
            assert_eq!(receipt.reads.len(), 8);
            assert_eq!(
                receipt.firmware_version,
                match receipt.device_id {
                    1 | 2 => [0, 3, 1, 42],
                    3 | 4 => [0, 2, 3, 34],
                    _ => [0, 0, 3, 32],
                }
            );
            assert_eq!(receipt.identity_wire_bytes, [receipt.device_id; 8]);
        }
        assert_eq!(owner.mode(), OperationalMode::Disabled);
        assert!(owner.set_homing_complete().is_err());
        assert!(owner
            .bus
            .tx
            .iter()
            .all(|frame| !matches!((frame.id >> 24) & 0x1F, 3 | 6)));
        assert!(
            owner.motor_states.is_empty(),
            "version bytes never renew pose"
        );
    }

    #[test]
    fn peer_fault_wins_over_successful_version_reply() {
        let mut owner = supervisor(ProbeBus {
            inject_peer_fault: true,
            ..ProbeBus::default()
        });
        assert!(owner.inspect_drive_protocol().is_err());
        assert!(owner.has_latched_fault());
        assert_eq!(owner.mode(), OperationalMode::Disabled);
        assert!(owner
            .bus
            .tx
            .iter()
            .all(|frame| (frame.id >> 24) & 0x1F != 3));
        assert!(
            owner
                .bus
                .tx
                .iter()
                .filter(|frame| (frame.id >> 24) & 0x1F == 4)
                .count()
                >= 10
        );
    }

    #[test]
    fn wrong_host_reply_cannot_complete_inspection() {
        let mut owner = supervisor(ProbeBus {
            wrong_host: true,
            ..ProbeBus::default()
        });
        let error = owner.inspect_drive_protocol().expect_err("host mismatch");
        assert!(error.to_string().contains("timed out"));
        assert_eq!(owner.mode(), OperationalMode::Disabled);
        assert!(owner.motor_states.is_empty());
    }

    #[test]
    fn standalone_factory_and_cleanup_never_reassert_reporting_on() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut owner = Supervisor::from_repo_for_protocol_inspection(root, ProbeBus::default())
            .expect("owner");
        assert!(owner.bus.tx.is_empty(), "constructor must not transmit");
        owner.inspect_drive_protocol().expect("inspection");
        for frame in &owner.bus.tx {
            if (frame.id >> 24) & 0x1F == 24 {
                assert_eq!(frame.data[6], 0);
            }
        }
    }

    #[test]
    fn failed_initial_stop_still_attempts_final_all_address_stop() {
        let mut owner = supervisor(ProbeBus {
            fail_initial_stop: true,
            ..ProbeBus::default()
        });
        assert!(owner.inspect_drive_protocol().is_err());
        assert!(owner.has_latched_fault());
        assert_eq!(
            owner
                .bus
                .tx
                .iter()
                .filter(|frame| (frame.id >> 24) & 0x1F == 4)
                .count(),
            10
        );
    }

    #[test]
    fn diagnostic_send_failure_latches_transport_before_cleanup() {
        let mut owner = supervisor(ProbeBus {
            fail_query: true,
            ..ProbeBus::default()
        });
        assert!(owner.inspect_drive_protocol().is_err());
        assert!(owner.has_latched_fault());
        assert_eq!(owner.mode(), OperationalMode::Disabled);
        assert!(owner.set_homing_complete().is_err());
    }

    #[test]
    fn conflicting_same_batch_and_later_version_modes_refuse_receipt() {
        for late in [false, true] {
            let mut owner = supervisor(ProbeBus {
                conflicting_mode: !late,
                late_conflict: late,
                ..ProbeBus::default()
            });
            let error = owner.inspect_drive_protocol().expect_err("contradiction");
            assert!(
                error.to_string().contains("conflicting")
                    || error.to_string().contains("unexpected drive mode")
            );
            assert_eq!(owner.mode(), OperationalMode::Disabled);
        }
    }

    #[test]
    fn bench_home_qualification_requires_explicit_confirmation_before_any_writes() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut owner = Supervisor::from_repo_for_protocol_inspection(root, ProbeBus::default())
            .expect("owner");
        assert!(owner
            .qualify_bench_home_disabled(false, true, "bench")
            .is_err());
        assert!(owner
            .qualify_bench_home_disabled(true, false, "bench")
            .is_err());
        assert!(owner.bus.tx.is_empty());
    }

    #[test]
    fn bench_home_qualification_readbacks_never_grant_or_enable() {
        let mut owner = supervisor(ProbeBus::default());
        let receipt = owner
            .qualify_bench_home_disabled(true, true, "bench")
            .expect("qualification");
        assert!(!receipt.grants_reference);
        assert_eq!(receipt.reads.len(), 20);
        assert_eq!(
            owner
                .bus
                .tx
                .iter()
                .filter(|frame| (frame.id >> 24) & 0x1F == 6)
                .count(),
            5
        );
        assert!(owner
            .bus
            .tx
            .iter()
            .all(|frame| (frame.id >> 24) & 0x1F != 3));
        assert!(owner.set_homing_complete().is_err());
        assert_eq!(owner.mode(), OperationalMode::Disabled);
    }

    #[test]
    fn missing_timeout_readback_and_bad_post_zero_pose_refuse_qualification() {
        for bad_zero in [false, true] {
            let mut owner = supervisor(ProbeBus {
                bad_zero,
                ignore_timeout_write: !bad_zero,
                ..ProbeBus::default()
            });
            assert!(owner
                .qualify_bench_home_disabled(true, true, "bench")
                .is_err());
            assert!(owner
                .bus
                .tx
                .iter()
                .all(|frame| (frame.id >> 24) & 0x1F != 3));
            assert_eq!(owner.mode(), OperationalMode::Disabled);
        }
    }

    #[test]
    fn observed_run_before_zero_latches_and_sends_no_set_zero() {
        let mut owner = supervisor(ProbeBus {
            inject_run_before_zero: true,
            ..ProbeBus::default()
        });
        let error = owner
            .qualify_bench_home_disabled(true, true, "bench")
            .expect_err("drive running");
        assert!(error.to_string().contains("unexpected drive mode"));
        assert!(owner.has_latched_fault());
        assert!(owner
            .bus
            .tx
            .iter()
            .all(|frame| (frame.id >> 24) & 0x1F != 6));
        assert_eq!(owner.mode(), OperationalMode::Disabled);
    }
}
