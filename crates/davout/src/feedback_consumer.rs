//! Private ordered feedback consumption; callers own lifecycle finalization.
//!
//! Shared hazard/chronology policy; operational and transaction owners finish stop handling.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use armee_kinematics::measured_position_fault;
use marengo_config::MotorEntry;
use robstride::{
    BusError, DriveMode, EnableEchoObservation, FeedbackEvent, FeedbackObservation, FeedbackReport,
    MalformedFeedback, MotorAddress, MotorBus, MotorState, RxFrameKind, TimedCanFrame,
};
use tracing::{debug, warn};

use super::{
    is_motor_address, motor_to_joint_state, store_by_joint, validate_motor_feedback, DavoutError,
    DeviceFaultEvidence, FaultClass, FeedbackSample, OperationalMode, ReceiveDrainEvidence,
    ReceiveFaultEvidence, ReceiveFrameEvidence, Supervisor, FEEDBACK_VELOCITY_FAULT_MARGIN_RAD_S,
    FEEDBACK_VELOCITY_LIMIT_TRIPS,
};

/// Private scope data, never output permission or a successful reference receipt.
/// The reference owner builds this only after validating its captured reservation.
#[derive(Debug)]
pub(super) enum ReceiveContext {
    Operational,
    Reference(ReferenceReceiveContext),
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ReferenceReceivePhase {
    BeforeEnable,
    /// Target armed, SetZero not yet attempted: its old coordinate is untrusted.
    EnabledBeforeZero,
    Enabled,
}

#[derive(Debug)]
pub(super) struct ReferenceReceiveContext {
    target: MotorAddress,
    phase: ReferenceReceivePhase,
}

impl ReferenceReceiveContext {
    /// `installed_target` must come from the reservation's immutable mapping.
    /// Calling this does not qualify capability, SetZero correlation or permission.
    pub(super) fn from_installed_target(
        installed_target: &MotorEntry,
        phase: ReferenceReceivePhase,
    ) -> Self {
        Self {
            target: MotorAddress::from(installed_target),
            phase,
        }
    }

    fn is_enabled(&self) -> bool {
        matches!(
            self.phase,
            ReferenceReceivePhase::EnabledBeforeZero | ReferenceReceivePhase::Enabled
        )
    }

    /// Measured-limit faults require a trusted coordinate: the target only after
    /// its SetZero attempt, peers only while they hold current permission.
    fn coordinate_trusted(&self, address: &MotorAddress, peer_referenced: bool) -> bool {
        if *address == self.target {
            matches!(self.phase, ReferenceReceivePhase::Enabled)
        } else {
            peer_referenced
        }
    }
}

impl ReceiveContext {
    fn reference_target(&self) -> Option<&MotorAddress> {
        match self {
            Self::Operational => None,
            Self::Reference(context) => Some(&context.target),
        }
    }
}

/// Chronologically admitted raw joint-space projection, before derivative replacement.
/// The selected address also passed coalesced velocity/cache admission. Only the
/// owner can join it to a sealed pop tag after checking the whole report outcome.
#[derive(Debug)]
pub(super) struct ReferencePoseObservation {
    pub(super) order: usize,
    pub(super) address: MotorAddress,
    pub(super) can_id: u32,
    pub(super) received_at: Instant,
    pub(super) state: MotorState,
}

/// One raw acquisition allowance per owner advance. Finite frame/read totals are
/// structural: a second acquisition fails without touching the transport.
pub(super) struct AdvanceReceiveBudget {
    unused: Option<robstride::ReceiveLimits>,
}

impl AdvanceReceiveBudget {
    pub(super) fn new(host_deadline: Option<Instant>) -> Self {
        Self {
            unused: Some(robstride::ReceiveLimits {
                max_frames: robstride::MAX_RX_FRAMES_PER_POLL,
                max_attempts: robstride::MAX_RX_ATTEMPTS_PER_POLL,
                deadline: host_deadline,
            }),
        }
    }

    pub(super) fn acquire<B: MotorBus>(
        &mut self,
        bus: &mut B,
        types: &HashMap<MotorAddress, marengo_config::MotorType>,
    ) -> Result<FeedbackReport, BusError> {
        let limits = self.unused.take().ok_or(BusError::ReceiveIncomplete {
            completion: robstride::ReceiveCompletion::WorkLimit,
        })?;
        // The default raw engine path deliberately excludes SimulationBus's
        // impossible-wire queue_feedback_report fixture override.
        Ok(bus.recv_feedback_report_with_limits(
            types,
            std::time::Duration::ZERO,
            std::time::Duration::ZERO,
            limits,
        ))
    }
}

/// Every caller must finish the safety lifecycle before using any admitted pose.
#[must_use]
pub(super) struct FeedbackConsumption {
    pub(super) first_error: Option<DavoutError>,
    pub(super) first_transition: bool,
    pub(super) reference_poses: Vec<ReferencePoseObservation>,
    pub(super) completion: robstride::ReceiveCompletion,
    pub(super) raw_frames: usize,
    pub(super) read_attempts: usize,
}

/// Latest admitted pose for one installed address within a drain.
#[derive(Debug)]
struct PoseCandidate {
    order: usize,
    /// Index of the first `stop_motors` entry routed to `address`.
    motor: usize,
    address: MotorAddress,
    state: MotorState,
}

enum OrderedReceive {
    Motor(FeedbackObservation),
    EnableEcho(EnableEchoObservation),
    Transport(TimedCanFrame),
    Terminal(BusError),
}

/// Reused consumption buffers; empty between reports.
#[derive(Default)]
pub(super) struct ConsumeScratch {
    ordered: Vec<(usize, u8, OrderedReceive)>,
    candidates: Vec<PoseCandidate>,
}

impl<B: MotorBus> Supervisor<B> {
    /// Run is required only for frames read after this address's Enable reached
    /// the wire. Pop time alone cannot show that: a write only queues the frame,
    /// so on an echoing bus the strict check arms at the Enable's echo, exactly
    /// as reference replies count only when popped after their request.
    fn feedback_run_expected(
        &self,
        motor: &MotorEntry,
        address: &MotorAddress,
        received_at: Instant,
        context: &ReceiveContext,
    ) -> bool {
        let on_wire = !self.enable_echo_pending.contains(address);
        match context {
            ReceiveContext::Operational => {
                self.mode == OperationalMode::Active
                    && self.active_joints.contains(&motor.joint)
                    && self
                        .active_since
                        .is_some_and(|enabled| received_at > enabled)
                    && on_wire
            }
            ReceiveContext::Reference(reference) => {
                reference.is_enabled() && *address == reference.target && on_wire
            }
        }
    }

    fn feedback_motion_guards_enabled(&self, context: &ReceiveContext) -> bool {
        match context {
            ReceiveContext::Operational => self.mode == OperationalMode::Active,
            // Like existing Active policy, inspect all installed peers' measured
            // position/velocity hazards once any reference target is armed.
            ReceiveContext::Reference(reference) => reference.is_enabled(),
        }
    }

    /// A guarded SetZero attempt may change the coordinate even if TX is uncertain.
    /// Invalidate only this installed target's derivative/freshness scratch before
    /// the attempt. This never clears fault authority, invalid-feedback evidence
    /// or a peer's history, and never qualifies a successful reference epoch.
    pub(super) fn invalidate_reference_target_pose_for_zero_attempt(
        &mut self,
        target: &MotorEntry,
    ) {
        let address = MotorAddress::from(target);
        self.feedback_velocity_trips.remove(&target.joint);
        self.last_feedback_samples.remove(&target.joint);
        self.last_feedback_rx.remove(&target.joint);
        self.motor_states.remove(&address);
    }

    pub(super) fn consume_feedback_report(
        &mut self,
        report: FeedbackReport,
    ) -> FeedbackConsumption {
        self.consume_report_in_context(report, &ReceiveContext::Operational)
    }

    /// Consume the complete ordered hazard stream without transmitting or stopping.
    /// Reference observations are raw joint-space projections; they carry no proof.
    pub(super) fn consume_report_in_context(
        &mut self,
        mut report: FeedbackReport,
        context: &ReceiveContext,
    ) -> FeedbackConsumption {
        // Physical owners retain identity/readback replies for request correlation
        // and identity continuity. Replies are never pose or permission evidence.
        if self.reference_owner.is_physical() {
            for identity in std::mem::take(&mut report.identities) {
                self.reference_owner.physical.record_identity(identity);
            }
            for read in std::mem::take(&mut report.parameter_reads) {
                self.reference_owner.physical.record_read(read);
            }
        }
        let at_rest_tolerance = self.homing_config.homing.zero_verify_tolerance_rad;
        let mut reference_poses = Vec::new();
        let mut first_error = None;
        let mut first_transition = false;
        // Installed routes are immutable; a shared handle lets `&mut self` hazard
        // handling borrow the matched entry without cloning it.
        let stop_motors = Arc::clone(&self.stop_motors);
        let mut scratch = std::mem::take(&mut self.feedback_scratch);
        let ConsumeScratch {
            ordered,
            candidates: pose_candidates,
        } = &mut scratch;
        ordered.clear();
        pose_candidates.clear();
        let mut clock_floor = None;
        ordered.extend(
            report
                .observations
                .into_iter()
                .map(|observation| (observation.order, 1_u8, OrderedReceive::Motor(observation))),
        );
        ordered.extend(report.transport_frames.into_iter().map(|observation| {
            (
                observation.order,
                1,
                OrderedReceive::Transport(observation.frame),
            )
        }));
        ordered.extend(
            report
                .enable_echoes
                .into_iter()
                .map(|echo| (echo.order, 1, OrderedReceive::EnableEcho(echo))),
        );
        if let Some(error) = report.terminal_error {
            // Actual backends supply the first error's raw position. Scripted reports
            // without an ordinal put the terminal failure after their delivered prefix.
            let order = report.terminal_error_order.unwrap_or_else(|| {
                ordered
                    .iter()
                    .map(|(order, _, _)| order.saturating_add(1))
                    .max()
                    .unwrap_or(0)
                    .max(report.raw_frames)
            });
            // A backend error preceded any later frame with this raw position.
            ordered.push((order, 0, OrderedReceive::Terminal(error)));
        }
        ordered.sort_by_key(|(order, rank, _)| (*order, *rank));
        // Do not return early: every available peer fault must reach authority,
        // even if another pose is invalid or the drain ends in a transport error.
        for (order, _, event) in ordered.drain(..) {
            let observation = match event {
                OrderedReceive::Motor(observation) => observation,
                OrderedReceive::EnableEcho(echo) => {
                    // Wire-order marker only: never pose, liveness or a reply.
                    self.enable_echo_pending.remove(&echo.address);
                    continue;
                }
                OrderedReceive::Terminal(error) => {
                    if !matches!(error, BusError::RecvTimeout) {
                        let error = DavoutError::Bus(error);
                        first_transition |= self.record_runtime_error(&error);
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                    continue;
                }
                OrderedReceive::Transport(frame) => {
                    // Error envelopes belong to transport, never a vendor device-ID decoder.
                    let evidence = ReceiveFrameEvidence {
                        interface: frame.received.interface,
                        can_id: frame.received.frame.id,
                        extended: frame.received.frame.extended,
                        received_at: frame.received_at,
                        raw: frame.received.frame.data,
                        payload_len: frame.received.payload_len,
                        kind: frame.received.kind,
                        reason: None,
                    };
                    first_transition |= self.fault_authority.record_receive(
                        FaultClass::Transport,
                        None,
                        None,
                        "received CAN error frame; physical bus state and error subscriptions unqualified",
                        DeviceFaultEvidence::default(),
                        ReceiveFaultEvidence::frame(evidence),
                    );
                    if first_error.is_none() {
                        first_error = self.require_fault_clear().err();
                    }
                    continue;
                }
            };
            let address = observation.address;
            let Some(motor_index) = stop_motors
                .iter()
                .position(|m| is_motor_address(&address, m))
            else {
                continue;
            };
            let motor = &stop_motors[motor_index];
            let mut device = DeviceFaultEvidence {
                received_at: Some(observation.received_at),
                ..DeviceFaultEvidence::default()
            };
            let status = match observation.event {
                FeedbackEvent::Malformed(malformed) => {
                    first_transition |= self.record_malformed_feedback(
                        motor,
                        &address,
                        observation.received_at,
                        observation.can_id,
                        malformed,
                        context,
                    );
                    if first_error.is_none() {
                        first_error = self.require_fault_clear().err();
                    }
                    continue;
                }
                FeedbackEvent::DetailedFault(report) => {
                    device.detailed_fault_bytes = report.fault_bytes();
                    device.warning_bytes = report.warning_bytes();
                    self.fault_authority.record_warning(
                        &motor.joint,
                        &address,
                        device.warning_bytes,
                    );
                    if report.has_fault() {
                        first_transition |= self.fault_authority.record(
                            FaultClass::Device,
                            Some(motor.joint.clone()),
                            Some(address),
                            "vendor detailed fault; word byte order and physical recovery unqualified",
                            device,
                        );
                        if first_error.is_none() {
                            first_error = self.require_fault_clear().err();
                        }
                    }
                    continue;
                }
                FeedbackEvent::Status(status) => {
                    device.status_flags = status.status_flags;
                    device.drive_mode = Some(match status.drive_mode {
                        DriveMode::Reset => 0,
                        DriveMode::Calibration => 1,
                        DriveMode::Run => 2,
                        DriveMode::Reserved => 3,
                    });
                    if status.drive_mode == DriveMode::Calibration {
                        // Encoder recalibration: the coordinate is no longer continuous.
                        self.reference_owner
                            .physical
                            .record_calibration_mode(&address);
                    }
                    status
                }
            };
            if device.status_flags != 0 {
                first_transition |= self.fault_authority.record(
                    FaultClass::Device,
                    Some(motor.joint.clone()),
                    Some(address.clone()),
                    "vendor status fault flags",
                    device.clone(),
                );
                if first_error.is_none() {
                    first_error = self.require_fault_clear().err();
                }
            }
            let current_enable =
                self.feedback_run_expected(motor, &address, observation.received_at, context);
            if status.drive_mode == DriveMode::Reserved
                || (current_enable && status.drive_mode != DriveMode::Run)
            {
                let error = DavoutError::InvalidFeedback {
                    joint: motor.joint.clone(),
                    message: format!("unexpected drive mode {:?} for {:?}; no qualified factory-calibration context", status.drive_mode, self.mode),
                };
                self.invalid_feedback.insert(address.clone());
                first_transition |= self.fault_authority.record(
                    FaultClass::DriveState,
                    Some(motor.joint.clone()),
                    Some(address),
                    &error.to_string(),
                    device,
                );
                if first_error.is_none() {
                    first_error = Some(error);
                }
                continue;
            }
            let raw = MotorState {
                position_rad: status.position_rad,
                velocity_rad_s: status.velocity_rad_s,
                torque_nm: status.torque_nm,
                temperature_c: status.temperature_c,
                fault: status.fault,
                updated: Some(observation.received_at),
            };
            let prepared = (|| {
                validate_motor_feedback(&motor.joint, &raw)?;
                let state = motor_to_joint_state(motor, raw)?;
                validate_motor_feedback(&motor.joint, &state)?;
                if is_future(observation.received_at, &mut clock_floor) {
                    return Err(DavoutError::InvalidFeedback {
                        joint: motor.joint.clone(),
                        message: "receive timestamp is in the future".into(),
                    });
                }
                // Hard-position evidence is inspected on every ordered frame,
                // even when two reads share a clock tick. Chronology only gates
                // pose renewal and derivative scratch, not hazard retention.
                match context {
                    ReceiveContext::Operational => self.check_feedback_position(motor, &state)?,
                    ReceiveContext::Reference(_) => {
                        self.check_feedback_position_in_context(motor, &state, context)?;
                    }
                }
                Ok(state)
            })();
            let state = match prepared {
                Ok(state) => state,
                Err(error) => {
                    self.invalid_feedback.insert(address);
                    first_transition |= self.record_runtime_error(&error);
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                    continue;
                }
            };
            // Read before this session's Enable reached the wire: hazards above
            // were inspected, but it cannot become post-enable session pose.
            if matches!(context, ReceiveContext::Operational)
                && self.mode == OperationalMode::Active
                && self.enable_echo_pending.contains(&address)
            {
                continue;
            }
            if self
                .motor_states
                .get(&address)
                .and_then(|state| state.updated)
                .is_some_and(|previous| previous >= observation.received_at)
            {
                continue;
            }
            let existing = pose_candidates
                .iter()
                .position(|candidate| candidate.address == address);
            if existing
                .and_then(|slot| pose_candidates[slot].state.updated)
                .is_some_and(|previous| previous > observation.received_at)
            {
                continue;
            }
            if context.reference_target() == Some(&address) && status.status_flags == 0 {
                reference_poses.push(ReferencePoseObservation {
                    order,
                    address: address.clone(),
                    can_id: observation.can_id,
                    received_at: observation.received_at,
                    state,
                });
            }
            let candidate = PoseCandidate {
                order,
                motor: motor_index,
                address,
                state,
            };
            match existing {
                Some(slot) => pose_candidates[slot] = candidate,
                None => pose_candidates.push(candidate),
            }
        }
        // Host read times cannot reconstruct the spacing of physical samples
        // queued before this drain. Inspect every raw hazard above, but update
        // position-derived velocity/trips only once per address per drain.
        pose_candidates.sort_unstable_by_key(|candidate| candidate.order);
        for PoseCandidate {
            motor,
            address,
            mut state,
            ..
        } in pose_candidates.drain(..)
        {
            let motor = &stop_motors[motor];
            let Some(received_at) = state.updated else {
                continue;
            };
            let velocity_result = match context {
                ReceiveContext::Operational => {
                    self.check_feedback_velocity(motor, &mut state, received_at)
                }
                ReceiveContext::Reference(_) => {
                    self.check_feedback_velocity_in_context(motor, &mut state, received_at, context)
                }
            };
            if let Err(error) =
                velocity_result.and_then(|()| validate_motor_feedback(&motor.joint, &state))
            {
                reference_poses.retain(|pose| pose.address != address);
                self.invalid_feedback.insert(address);
                first_transition |= self.record_runtime_error(&error);
                if first_error.is_none() {
                    first_error = Some(error);
                }
                continue;
            }
            self.invalid_feedback.remove(&address);
            if self.reference_owner.is_physical() {
                self.reference_owner.physical.observe_position(
                    &address,
                    f64::from(state.position_rad),
                    received_at,
                    at_rest_tolerance,
                );
            }
            self.motor_states.insert(address, state);
            store_by_joint(&mut self.last_feedback_rx, &motor.joint, received_at);
        }
        self.feedback_scratch = scratch;
        if !report.completion.is_complete() {
            let error = DavoutError::Bus(BusError::ReceiveIncomplete {
                completion: report.completion,
            });
            let message = format!(
                "{error}; raw frames={}, read attempts={}",
                report.raw_frames, report.read_attempts
            );
            first_transition |= self.fault_authority.record_receive(
                FaultClass::Transport,
                None,
                None,
                &message,
                DeviceFaultEvidence::default(),
                ReceiveFaultEvidence::incomplete(ReceiveDrainEvidence {
                    completion: report.completion,
                    raw_frames: report.raw_frames,
                    read_attempts: report.read_attempts,
                }),
            );
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
        FeedbackConsumption {
            first_error,
            first_transition,
            reference_poses,
            completion: report.completion,
            raw_frames: report.raw_frames,
            read_attempts: report.read_attempts,
        }
    }

    fn record_malformed_feedback(
        &mut self,
        motor: &MotorEntry,
        address: &MotorAddress,
        received_at: Instant,
        can_id: u32,
        malformed: MalformedFeedback,
        context: &ReceiveContext,
    ) -> bool {
        let mut device = DeviceFaultEvidence {
            received_at: Some(received_at),
            ..DeviceFaultEvidence::default()
        };
        // RTR identifiers do not constitute vendor status/fault proof. Only a Data
        // envelope can provide qualified header bits or actually received prefixes.
        if malformed.kind == RxFrameKind::Data {
            device.status_flags = malformed.status_flags.unwrap_or(0);
            device.drive_mode = malformed.drive_mode.map(|mode| match mode {
                DriveMode::Reset => 0,
                DriveMode::Calibration => 1,
                DriveMode::Run => 2,
                DriveMode::Reserved => 3,
            });
            if can_id >> 24 == 21 {
                let length = usize::from(malformed.payload_len).min(8);
                for (index, byte) in malformed.raw.iter().copied().take(length).enumerate() {
                    if index < 4 {
                        device.partial_fault_bytes[index] = byte;
                        device.partial_fault_available |= 1 << index;
                    } else {
                        device.partial_warning_bytes[index - 4] = byte;
                        device.partial_warning_available |= 1 << (index - 4);
                    }
                }
            }
        }
        let mut first = false;
        if device.status_flags != 0 || device.partial_fault_bytes != [0; 4] {
            first |= self.fault_authority.record(
                FaultClass::Device,
                Some(motor.joint.clone()),
                Some(address.clone()),
                "vendor fault evidence from malformed data; partial words remain unqualified",
                device.clone(),
            );
        }
        let current_enable = self.feedback_run_expected(motor, address, received_at, context);
        if device.drive_mode == Some(3)
            || (current_enable && device.drive_mode.is_some_and(|mode| mode != 2))
        {
            first |= self.fault_authority.record(
                FaultClass::DriveState,
                Some(motor.joint.clone()),
                Some(address.clone()),
                "unexpected drive mode in malformed Data status",
                device.clone(),
            );
        }
        self.invalid_feedback.insert(address.clone());
        let error = DavoutError::Bus(BusError::MalformedFeedback {
            address: address.clone(),
            reason: malformed.reason,
        });
        first |= self.fault_authority.record_receive(
            FaultClass::Feedback,
            Some(motor.joint.clone()),
            Some(address.clone()),
            &error.to_string(),
            device,
            ReceiveFaultEvidence::frame(ReceiveFrameEvidence {
                interface: Some(address.interface.clone()),
                can_id,
                // Configured vendor observations are recognized extended frames.
                extended: true,
                received_at,
                raw: malformed.raw,
                payload_len: malformed.payload_len,
                kind: malformed.kind,
                reason: Some(malformed.reason),
            }),
        );
        first
    }

    pub(super) fn check_feedback_velocity(
        &mut self,
        motor: &MotorEntry,
        state: &mut MotorState,
        received_at: Instant,
    ) -> Result<(), DavoutError> {
        self.check_feedback_velocity_in_context(
            motor,
            state,
            received_at,
            &ReceiveContext::Operational,
        )
    }

    fn check_feedback_velocity_in_context(
        &mut self,
        motor: &MotorEntry,
        state: &mut MotorState,
        received_at: Instant,
        context: &ReceiveContext,
    ) -> Result<(), DavoutError> {
        if !self.feedback_motion_guards_enabled(context) {
            self.feedback_velocity_trips.remove(&motor.joint);
            self.last_feedback_samples.remove(&motor.joint);
            return Ok(());
        }
        let lim = self
            .limits
            .get(&motor.joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: motor.joint.clone(),
            })?;
        let raw_velocity = f64::from(state.velocity_rad_s);
        let position = f64::from(state.position_rad);
        let previous = self.last_feedback_samples.get(&motor.joint).copied();
        let position_velocity = previous.and_then(|prev| {
            let dt = received_at.duration_since(prev.received_at).as_secs_f64();
            (dt > 0.0).then_some((position - prev.position_rad) / dt)
        });
        let measured_velocity = position_velocity.unwrap_or(raw_velocity);
        state.velocity_rad_s = measured_velocity as f32;
        let fault_threshold = lim.velocity + FEEDBACK_VELOCITY_FAULT_MARGIN_RAD_S;

        if raw_velocity.abs() > lim.velocity && measured_velocity.abs() <= fault_threshold {
            debug!(
                joint = %motor.joint,
                position_rad = position,
                previous_position_rad = previous.map(|prev| prev.position_rad),
                raw_velocity_rad_s = raw_velocity,
                measured_velocity_rad_s = measured_velocity,
                fault_threshold_rad_s = fault_threshold,
                limit_rad_s = lim.velocity,
                "ignored uncorroborated feedback velocity spike"
            );
            self.feedback_velocity_trips.remove(&motor.joint);
            store_by_joint(
                &mut self.last_feedback_samples,
                &motor.joint,
                FeedbackSample {
                    position_rad: position,
                    received_at,
                },
            );
            return Ok(());
        }

        if measured_velocity.abs() > fault_threshold {
            let trips = self
                .feedback_velocity_trips
                .entry(motor.joint.clone())
                .and_modify(|count| *count = count.saturating_add(1))
                .or_insert(1);
            warn!(
                joint = %motor.joint,
                position_rad = position,
                previous_position_rad = previous.map(|prev| prev.position_rad),
                raw_velocity_rad_s = raw_velocity,
                measured_velocity_rad_s = measured_velocity,
                fault_threshold_rad_s = fault_threshold,
                limit_rad_s = lim.velocity,
                trips = *trips,
                "feedback velocity limit exceeded"
            );
            store_by_joint(
                &mut self.last_feedback_samples,
                &motor.joint,
                FeedbackSample {
                    position_rad: position,
                    received_at,
                },
            );
            if *trips >= FEEDBACK_VELOCITY_LIMIT_TRIPS {
                return Err(DavoutError::Limit {
                    joint: motor.joint.clone(),
                    message: format!(
                        "feedback |velocity| {measured_velocity} > {fault_threshold} (limit {} + margin {})",
                        lim.velocity, FEEDBACK_VELOCITY_FAULT_MARGIN_RAD_S
                    ),
                });
            }
        } else {
            self.feedback_velocity_trips.remove(&motor.joint);
        }
        store_by_joint(
            &mut self.last_feedback_samples,
            &motor.joint,
            FeedbackSample {
                position_rad: position,
                received_at,
            },
        );
        Ok(())
    }

    pub(super) fn check_feedback_position(
        &mut self,
        motor: &MotorEntry,
        state: &MotorState,
    ) -> Result<(), DavoutError> {
        self.check_feedback_position_in_context(motor, state, &ReceiveContext::Operational)
    }

    fn check_feedback_position_in_context(
        &mut self,
        motor: &MotorEntry,
        state: &MotorState,
        context: &ReceiveContext,
    ) -> Result<(), DavoutError> {
        if !self.feedback_motion_guards_enabled(context) {
            return Ok(());
        }
        if let ReceiveContext::Reference(reference) = context {
            let address = MotorAddress::from(motor);
            if !reference
                .coordinate_trusted(&address, self.reference_authority.contains(&motor.joint))
            {
                return Ok(());
            }
        }
        let lim = self
            .limits
            .get(&motor.joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: motor.joint.clone(),
            })?;
        let position = f64::from(state.position_rad);
        if measured_position_fault(position, lim) {
            self.homing.mark_out_of_limits(&motor.joint);
            return Err(DavoutError::Limit {
                joint: motor.joint.clone(),
                message: format!(
                    "measured position {position} outside [{}, {}] (+ slack {})",
                    lim.hard_lower(),
                    lim.hard_upper(),
                    lim.margin.measured_fault_slack_rad
                ),
            });
        }
        Ok(())
    }
}

/// `received_at > Instant::now()`, reading the clock only when `received_at` is
/// past `floor` (the last read). `Instant` is monotonic, so a fresh read could
/// never make an instant at or before `floor` lie in the future.
fn is_future(received_at: Instant, floor: &mut Option<Instant>) -> bool {
    if floor.is_some_and(|floor| received_at <= floor) {
        return false;
    }
    let now = Instant::now();
    *floor = Some(now);
    received_at > now
}
