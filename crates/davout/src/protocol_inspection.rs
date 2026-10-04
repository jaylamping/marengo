//! Disabled drive protocol inspection (ADR 0037): read-only queries to stopped
//! drives for a standalone CLI owner. Never called from the control tick.
//!
//! The only frames written are type-4 Disable (Byte[0] = 0, never a fault
//! clear), type-24 reporting Off and the read queries themselves: type-4 with
//! the `C4` firmware version selector, type-0 device identity and type-17
//! parameter reads, all on [`robstride::DEFAULT_HOST_ID`]. Nothing here can
//! Enable, Set Zero, write or save a parameter, or create Ready or a
//! current-reference grant.

use std::sync::Arc;
use std::time::{Duration, Instant};

use robstride::{
    DeviceUid, FeedbackEvent, FeedbackReport, FirmwareVersion, FirmwareVersionFeedback,
    MotorAddress, MotorBus, ParameterId, ParameterReadReply, ParameterValue, DEFAULT_HOST_ID,
};

use super::burst::BurstPacer;
use super::faults::bounded_message;
use super::feedback_consumer::ReceiveContext;
use super::{
    DavoutError, DeviceFaultEvidence, FaultClass, OperationalMode, StopAction, StopAttempt,
    StopReport, Supervisor,
};

/// Registers read from every inspected drive, in this order. 0x7028 is the
/// drive-side CAN timeout (20000 counts = 1 s; 0 = off).
pub const INSPECTED_PARAMETERS: [ParameterId; 6] = [
    ParameterId::RunMode,
    ParameterId::MechPos,
    ParameterId::MechVel,
    ParameterId::CanTimeout,
    ParameterId::ZeroSta,
    ParameterId::AddOffset,
];

/// Longest wait for one query's reply, from its write.
pub const INSPECTION_QUERY_TIMEOUT: Duration = Duration::from_millis(300);

/// Receive window after the opening stop and after the last reply: the stop's
/// own replies and any late or duplicate reply are judged inside it.
pub const INSPECTION_SETTLE: Duration = Duration::from_millis(50);

const INSPECTION_POLL: Duration = Duration::from_millis(1);

/// Replies of one Disabled drive. Values are exactly what the drive returned.
#[derive(Debug, Clone, PartialEq)]
pub struct DriveProtocolInspection {
    pub joint: String,
    pub address: MotorAddress,
    pub firmware: FirmwareVersion,
    pub uid: DeviceUid,
    /// One successful read per [`INSPECTED_PARAMETERS`] entry, in that order.
    pub parameters: Vec<(ParameterId, ParameterValue)>,
}

impl DriveProtocolInspection {
    pub fn parameter(&self, parameter: ParameterId) -> Option<ParameterValue> {
        self.parameters
            .iter()
            .find(|(id, _)| *id == parameter)
            .map(|(_, value)| *value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Query {
    Version,
    Identity,
    Read(ParameterId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    Version(FirmwareVersionFeedback),
    Identity(DeviceUid),
    Read(ParameterReadReply),
    /// A type-17 reply naming a register this driver does not know.
    UnknownRead(ParameterReadReply),
}

/// One reply popped from the bus, before correlation.
#[derive(Debug, Clone)]
struct Reply {
    address: MotorAddress,
    received_at: Instant,
    can_id: u32,
    answer: Answer,
}

impl Reply {
    fn query(&self) -> Option<Query> {
        match self.answer {
            Answer::Version(_) => Some(Query::Version),
            Answer::Identity(_) => Some(Query::Identity),
            Answer::Read(reply) => reply.parameter().map(Query::Read),
            Answer::UnknownRead(_) => None,
        }
    }

    /// The same wire answer, including status bits in the identifier.
    fn agrees_with(&self, other: &Self) -> bool {
        self.can_id == other.can_id && self.answer == other.answer
    }
}

/// The query awaiting its reply.
struct Pending<'a> {
    address: &'a MotorAddress,
    query: Query,
    issued: Instant,
}

fn replies_in(report: &FeedbackReport) -> Vec<Reply> {
    let mut replies = Vec::new();
    for observation in &report.observations {
        if let FeedbackEvent::FirmwareVersion(version) = observation.event {
            replies.push(Reply {
                address: observation.address.clone(),
                received_at: observation.received_at,
                can_id: observation.can_id,
                answer: Answer::Version(version),
            });
        }
    }
    for identity in &report.identities {
        replies.push(Reply {
            address: identity.address.clone(),
            received_at: identity.received_at,
            can_id: identity.can_id,
            answer: Answer::Identity(identity.uid),
        });
    }
    for read in &report.parameter_reads {
        replies.push(Reply {
            address: read.address.clone(),
            received_at: read.received_at,
            can_id: read.can_id,
            answer: if read.reply.parameter().is_some() {
                Answer::Read(read.reply)
            } else {
                Answer::UnknownRead(read.reply)
            },
        });
    }
    replies
}

fn refusal(message: String) -> DavoutError {
    DavoutError::ProtocolInspection { message }
}

impl<B: MotorBus> Supervisor<B> {
    /// Owner for [`Self::inspect_drive_protocol`]: loads and validates like
    /// [`Self::from_repo`] but transmits nothing, so no type-24 On leaves at
    /// startup. Every startup is Unreferenced.
    pub fn from_repo_for_protocol_inspection(
        repo_root: impl AsRef<std::path::Path>,
        bus: B,
    ) -> Result<Self, DavoutError> {
        let root = repo_root.as_ref();
        let (owner, _) =
            Self::build_unsynced(root, &marengo_config::resolve_config_dir(root), bus)?;
        Ok(owner)
    }

    /// Read firmware version, MCU identity and [`INSPECTED_PARAMETERS`] from each
    /// listed joint's drive (all configured joints when `joints` is empty).
    ///
    /// Requires Disabled, no reference work, clear fault authority and no E-stop.
    /// Every installed address is stopped (Disable only) and its reporting
    /// turned Off before the first query, and stopped again on every result.
    /// Queries go one at a time, each in its own [`super::BURST_GROUP_SPACING`]
    /// group, so at most one reply is outstanding. Every report goes through the
    /// shared hazard consumer: a drive not in Reset, a status fault, a reply no
    /// query asked for, or two different replies to one query refuse the
    /// inspection. Blocking: at most [`INSPECTION_QUERY_TIMEOUT`] per query.
    pub fn inspect_drive_protocol(
        &mut self,
        joints: &[String],
    ) -> Result<Vec<DriveProtocolInspection>, DavoutError> {
        self.refuse_reference_interference("drive protocol inspection")?;
        self.require_fault_clear()?;
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        if self.mode != OperationalMode::Disabled {
            return Err(refusal(format!(
                "requires Disabled, owner is {:?}",
                self.mode
            )));
        }
        let targets = self.inspection_targets(joints)?;
        let mut pacer = BurstPacer::default();
        let result = self.inspect_stopped(&targets, &mut pacer);
        let cleanup = self.inspection_stop(&mut pacer);
        match (result, cleanup) {
            (result, Ok(())) => result,
            (Ok(_), Err(cleanup)) => Err(cleanup),
            (Err(error), Err(cleanup)) => Err(refusal(format!(
                "{error}; final stop also failed: {cleanup}"
            ))),
        }
    }

    fn inspection_targets(
        &self,
        joints: &[String],
    ) -> Result<Vec<(String, MotorAddress)>, DavoutError> {
        if joints.is_empty() {
            return Ok(self
                .motors
                .motors
                .iter()
                .map(|motor| (motor.joint.clone(), MotorAddress::from(motor)))
                .collect());
        }
        let mut targets: Vec<(String, MotorAddress)> = Vec::with_capacity(joints.len());
        for joint in joints {
            let motor = marengo_config::motor_for_joint(&self.motors, joint).ok_or_else(|| {
                DavoutError::UnknownJoint {
                    joint: joint.clone(),
                }
            })?;
            if targets.iter().any(|(listed, _)| listed == joint) {
                return Err(refusal(format!("joint {joint} listed twice")));
            }
            targets.push((joint.clone(), MotorAddress::from(motor)));
        }
        Ok(targets)
    }

    fn inspect_stopped(
        &mut self,
        targets: &[(String, MotorAddress)],
        pacer: &mut BurstPacer,
    ) -> Result<Vec<DriveProtocolInspection>, DavoutError> {
        // A drive a dead owner left enabled is Disabled before any query.
        self.inspection_stop(pacer)?;
        let stop_motors = Arc::clone(&self.stop_motors);
        for motor in stop_motors.iter() {
            pacer.begin_group(&motor.can_interface);
            let result = self
                .bus
                .disable_active_reporting_at(&MotorAddress::from(motor));
            self.inspection_sent(result)?;
        }
        let mut answered: Vec<Reply> = Vec::with_capacity(targets.len() * 8);
        // Drives may still answer from Run until the stop lands: settle under
        // the ordinary Disabled policy, then require Reset on everything.
        self.inspection_settle(&ReceiveContext::Operational, &answered)?;
        let mut inspections = Vec::with_capacity(targets.len());
        for (joint, address) in targets {
            let version =
                self.inspection_query(joint, address, Query::Version, pacer, &mut answered)?;
            let uid =
                self.inspection_query(joint, address, Query::Identity, pacer, &mut answered)?;
            let (Answer::Version(version), Answer::Identity(uid)) = (version, uid) else {
                return Err(refusal(format!(
                    "{joint}: reply kind does not match its query"
                )));
            };
            let mut parameters = Vec::with_capacity(INSPECTED_PARAMETERS.len());
            for parameter in INSPECTED_PARAMETERS {
                let Answer::Read(reply) = self.inspection_query(
                    joint,
                    address,
                    Query::Read(parameter),
                    pacer,
                    &mut answered,
                )?
                else {
                    return Err(refusal(format!(
                        "{joint}: {parameter:?} reply is not a read"
                    )));
                };
                let value = reply.typed_value().ok_or_else(|| {
                    refusal(format!("{joint}: {parameter:?} reply has no typed value"))
                })?;
                parameters.push((parameter, value));
            }
            inspections.push(DriveProtocolInspection {
                joint: joint.clone(),
                address: address.clone(),
                firmware: version.version,
                uid,
                parameters,
            });
        }
        self.inspection_settle(&ReceiveContext::DisabledInspection, &answered)?;
        Ok(inspections)
    }

    /// Write one query in its own pacing group and wait for its single reply.
    fn inspection_query(
        &mut self,
        joint: &str,
        address: &MotorAddress,
        query: Query,
        pacer: &mut BurstPacer,
        answered: &mut Vec<Reply>,
    ) -> Result<Answer, DavoutError> {
        pacer.begin_group(&address.interface);
        let issued = Instant::now();
        let result = match query {
            Query::Version => self.bus.get_firmware_version_at(address),
            Query::Identity => self.bus.get_device_id_at(address),
            Query::Read(parameter) => self.bus.read_parameter_at(address, parameter),
        };
        self.inspection_sent(result)?;
        let pending = Pending {
            address,
            query,
            issued,
        };
        let deadline = issued + INSPECTION_QUERY_TIMEOUT;
        loop {
            if let Some(reply) = self.inspection_drain(
                &ReceiveContext::DisabledInspection,
                answered,
                Some(&pending),
            )? {
                let answer = reply.answer;
                answered.push(reply);
                if let Answer::Read(read) = answer {
                    if !read.succeeded() {
                        return Err(refusal(format!(
                            "{joint}: {query:?} returned status {}",
                            read.status
                        )));
                    }
                }
                return Ok(answer);
            }
            if Instant::now() >= deadline {
                return Err(refusal(format!(
                    "{joint}: no {query:?} reply within {} ms",
                    INSPECTION_QUERY_TIMEOUT.as_millis()
                )));
            }
            std::thread::sleep(INSPECTION_POLL);
        }
    }

    /// Drain for [`INSPECTION_SETTLE`] with no query outstanding.
    fn inspection_settle(
        &mut self,
        context: &ReceiveContext,
        answered: &[Reply],
    ) -> Result<(), DavoutError> {
        let until = Instant::now() + INSPECTION_SETTLE;
        loop {
            self.inspection_drain(context, answered, None)?;
            if Instant::now() >= until {
                return Ok(());
            }
            std::thread::sleep(INSPECTION_POLL);
        }
    }

    /// One bounded drain. Every hazard is consumed before any reply is judged.
    /// Returns the pending query's reply, if it arrived.
    fn inspection_drain(
        &mut self,
        context: &ReceiveContext,
        answered: &[Reply],
        pending: Option<&Pending<'_>>,
    ) -> Result<Option<Reply>, DavoutError> {
        let report =
            self.bus
                .recv_feedback_report(&self.motor_types, Duration::ZERO, Duration::ZERO);
        let replies = replies_in(&report);
        let consumed = self.consume_report_in_context(report, context);
        if let Some(error) = consumed.first_error {
            return Err(error);
        }
        self.require_fault_clear()?;
        let mut answer: Option<Reply> = None;
        for reply in replies {
            let query = reply.query();
            if let Some(prior) = answered
                .iter()
                .find(|prior| prior.address == reply.address && prior.query() == query)
            {
                if !prior.agrees_with(&reply) {
                    return Err(self.refuse_reply(&reply, "contradicts the reply already accepted"));
                }
                continue;
            }
            let own_host = match reply.answer {
                Answer::Version(version) => version.host_id == DEFAULT_HOST_ID,
                _ => true,
            };
            let solicited = own_host
                && pending.is_some_and(|pending| {
                    *pending.address == reply.address
                        && Some(pending.query) == query
                        && reply.received_at >= pending.issued
                });
            if !solicited {
                return Err(self.refuse_reply(
                    &reply,
                    "answers no query of this inspection; is another CAN owner running?",
                ));
            }
            match &answer {
                Some(first) if !first.agrees_with(&reply) => {
                    return Err(
                        self.refuse_reply(&reply, "contradicts another reply to the same query")
                    );
                }
                Some(_) => {}
                None => answer = Some(reply),
            }
        }
        Ok(answer)
    }

    /// Latch a reply that cannot be trusted as this inspection's evidence.
    fn refuse_reply(&mut self, reply: &Reply, why: &str) -> DavoutError {
        let joint = self
            .stop_motors
            .iter()
            .find(|motor| MotorAddress::from(*motor) == reply.address)
            .map_or_else(
                || format!("{}:{}", reply.address.interface, reply.address.device_id),
                |motor| motor.joint.clone(),
            );
        let error = DavoutError::InvalidFeedback {
            joint,
            message: format!("protocol inspection reply {:#010x} {why}", reply.can_id),
        };
        self.record_runtime_error(&error);
        error
    }

    /// A failed write latches Transport, as any other failed send does.
    fn inspection_sent(
        &mut self,
        result: Result<(), robstride::BusError>,
    ) -> Result<(), DavoutError> {
        result.map_err(|error| {
            let error = DavoutError::Bus(error);
            self.record_runtime_error(&error);
            error
        })
    }

    /// All-address paced stop with Disable only: the canonical stop's zero-speed
    /// write and neutral MIT frame are not read frames, and a Disable alone
    /// leaves a drive in Reset. Every address is attempted.
    fn inspection_stop(&mut self, pacer: &mut BurstPacer) -> Result<(), DavoutError> {
        let generation = self.fault_authority.begin_stop();
        let stop_motors = Arc::clone(&self.stop_motors);
        let mut report = StopReport {
            generation,
            attempts: Vec::with_capacity(stop_motors.len()),
        };
        for motor in stop_motors.iter() {
            pacer.begin_group(&motor.can_interface);
            let address = MotorAddress::from(motor);
            let result = self.bus.disable_drive_at(&address);
            report.attempts.push(StopAttempt {
                address,
                action: StopAction::Disable,
                error: result
                    .err()
                    .map(|error| bounded_message(&error.to_string())),
            });
        }
        let failed_writes = report.failed_writes();
        if failed_writes > 0 {
            self.reference_authority.revoke();
            self.fault_authority.record(
                FaultClass::StopDelivery,
                None,
                None,
                "one or more stop writes failed; physical stop unconfirmed",
                DeviceFaultEvidence::default(),
            );
        }
        self.fault_authority.set_stop_report(report);
        if failed_writes > 0 {
            Err(DavoutError::StopDelivery { failed_writes })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "protocol_inspection_tests.rs"]
mod tests;
