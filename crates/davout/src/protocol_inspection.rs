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
enum Query {
    Identity,
    Version,
    Parameter(ParameterId),
}

impl Query {
    fn frame(self, host: u8, motor: u8) -> CanFrame {
        match self {
            Self::Identity => encode_device_identity_query(host, motor),
            Self::Version => encode_firmware_version_query(host, motor),
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
        self.disable_all()?;
        let result = self.inspect_drive_protocol_inner();
        let cleanup = self.disable_all();
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

    fn inspect_drive_protocol_inner(
        &mut self,
    ) -> Result<Vec<MotorProtocolInspection>, DavoutError> {
        let motors = self.motors.motors.clone();
        let mut receipts = Vec::with_capacity(motors.len());
        // Never reuse a query host within this finite inspection. UID replies have
        // a vendor-fixed FE destination and do not echo this discriminator.
        let mut host = 0x80_u8;
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
                let (reply, receipt) =
                    self.inspect_protocol_query(&motor.joint, &address, query, host)?;
                host = host
                    .checked_add(1)
                    .ok_or_else(|| DavoutError::ProtocolInspection {
                        joint: motor.joint.clone(),
                        message: "host discriminator exhausted".into(),
                    })?;
                match reply.reply {
                    ProtocolReply::FirmwareVersion(version) => {
                        inspection.firmware_version = version
                    }
                    ProtocolReply::DeviceIdentity(identity) => {
                        inspection.identity_wire_bytes = identity
                    }
                    ProtocolReply::Parameter { .. } => {}
                }
                inspection.reads.push(receipt);
            }
            receipts.push(inspection);
        }
        Ok(receipts)
    }

    fn inspect_protocol_query(
        &mut self,
        joint: &str,
        address: &MotorAddress,
        query: Query,
        host: u8,
    ) -> Result<(ProtocolObservation, ProtocolReadReceipt), DavoutError> {
        // Consume stale queued hazards before issuing the diagnostic request.
        self.drain_feedback()?;
        let request = query.frame(host, address.device_id);
        let issued = Instant::now();
        self.bus.send_frame_to(address, &request)?;
        let deadline = issued + QUERY_TIMEOUT;
        loop {
            let report =
                self.bus
                    .recv_feedback_report(&self.motor_types, Duration::ZERO, Duration::ZERO);
            let candidates: Vec<_> = report
                .protocol_observations
                .iter()
                .filter(|observation| {
                    observation.address == *address
                        && observation.received_at >= issued
                        && observation.received_at < deadline
                        && query.accepts(observation, host)
                })
                .cloned()
                .collect();
            let consumption = self.consume_feedback_report(report);
            if consumption.first_transition {
                let _ = self.disable_all();
            }
            if let Some(error) = consumption.first_error {
                return Err(error);
            }
            self.require_fault_clear()?;
            if Instant::now() >= deadline {
                return Err(DavoutError::ProtocolInspection {
                    joint: joint.into(),
                    message: format!("{} timed out", query.label()),
                });
            }
            if let Some(reply) = candidates.first() {
                if candidates.iter().any(|other| other.reply != reply.reply) {
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
                if matches!(query, Query::Version) && ((reply.can_id >> 22) & 3) != 0 {
                    return Err(DavoutError::ProtocolInspection {
                        joint: joint.into(),
                        message: "version reply did not confirm Reset/Disabled".into(),
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
    use std::collections::VecDeque;
    use std::path::PathBuf;

    #[derive(Default)]
    struct ProbeBus {
        tx: Vec<CanFrame>,
        rx: VecDeque<TimedCanFrame>,
        inject_peer_fault: bool,
        wrong_host: bool,
    }

    impl CanBus for ProbeBus {
        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), robstride::BusError> {
            self.tx.push(frame.clone());
            let id = robstride::unpack_ext_id(frame.id).expect("query ID");
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
                    data: [0, 0xC4, 0x56, 0, 3, 1, 42, 0],
                    extended: true,
                }),
                17 => Some(CanFrame {
                    id: robstride::pack_ext_id(17, u16::from(id.device_id), host),
                    data: [frame.data[0], frame.data[1], 0, 0, 0, 0, 0, 0],
                    extended: true,
                }),
                _ => None,
            };
            if let Some(response) = response {
                self.rx.push_back(TimedCanFrame {
                    received_at: Instant::now(),
                    received: ReceivedCanFrame::full_data(Some("can0".into()), response),
                });
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
            assert_eq!(receipt.firmware_version, [0, 3, 1, 42]);
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
}
