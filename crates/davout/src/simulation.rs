//! Closed, finite in-memory transport for software admission and output tests.
//!
//! This type cannot wrap a transport, socket or callback. The specialized
//! Supervisor constructor declares an INITIAL virtual reference fixture; it
//! does not verify acquisition, firmware SetZero correlation or physical safety.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use marengo_config::MotorType;
use robstride::{
    BusError, CanBus, CanFrame, FeedbackReport, MotorAddress, MotorBus, ReceiveAttempt,
    ReceivedCanFrame, TimedCanFrame,
};

use crate::{DavoutError, Supervisor};

const MAX_SCRIPT_ITEMS: usize = 16_384;
const MAX_TX_RULES: usize = 128;
const MAX_TRACE_ITEMS: usize = 65_536;

/// Explicit initial condition, never a successful reference transaction.
#[derive(Debug)]
pub enum InitialVirtualReference {
    Unreferenced,
    AllConfigured,
    Joints(Vec<String>),
}

#[derive(Debug, thiserror::Error)]
pub enum SimulationError {
    #[error("simulation script or trace capacity exceeded")]
    Capacity,
    #[error("simulation source count must be between one and 64")]
    SourceCount,
    #[error("simulation source selector is outside the installed source set")]
    Source,
    #[error("simulation transmit occurrence must be positive")]
    Occurrence,
    #[error("typed consumer report exceeds the bounded receive contract")]
    ReportCapacity,
}

/// Raw data is stamped on delivery; Timed preserves the supplied host timestamp.
/// Error is a finite diagnostic string, never a delegated receive function.
#[derive(Debug, Clone)]
pub enum SimulationReceive {
    Received(ReceivedCanFrame),
    Timed(TimedCanFrame),
    Idle,
    Interrupted,
    Error(String),
}

impl From<CanFrame> for SimulationReceive {
    fn from(frame: CanFrame) -> Self {
        Self::Received(ReceivedCanFrame::full_data(None, frame))
    }
}

#[derive(Debug, Default, Clone)]
pub struct TxMatcher {
    pub communication_type: Option<u8>,
    pub device_id: Option<u8>,
    pub interface: Option<String>,
}

impl TxMatcher {
    fn matches(&self, address: Option<&MotorAddress>, frame: &CanFrame) -> bool {
        self.communication_type
            .is_none_or(|kind| frame.id >> 24 == u32::from(kind))
            && self
                .device_id
                .is_none_or(|device| frame.id & 0xff == u32::from(device))
            && self.interface.as_ref().is_none_or(|interface| {
                address.is_some_and(|address| &address.interface == interface)
            })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TxOccurrence {
    Nth(usize),
    Every,
}

/// Declarative effect after recording a matching transmit attempt. Effects run
/// even on a scripted failed write, allowing explicit uncertainty coverage.
#[derive(Debug, Clone)]
pub struct TxRule {
    pub matcher: TxMatcher,
    pub occurrence: TxOccurrence,
    pub receive: Vec<SimulationReceive>,
    pub send_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleId(usize);

#[derive(Debug)]
struct InstalledRule {
    id: RuleId,
    rule: TxRule,
    matched: usize,
    triggered: usize,
    receive_source: usize,
}

/// One attempted write, including failed delivery. No physical acknowledgement.
#[derive(Debug, Clone)]
pub struct SimulationTransmission {
    pub address: Option<MotorAddress>,
    pub frame: CanFrame,
    pub delivered: bool,
}

/// Finite data-only simulation realm. No wrapped arbitrary MotorBus or conversion.
pub struct SimulationBus {
    realm: Arc<()>,
    receive: Vec<VecDeque<SimulationReceive>>,
    reports: VecDeque<FeedbackReport>,
    rules: Vec<InstalledRule>,
    next_rule: usize,
    transmissions: Vec<SimulationTransmission>,
    frames: Vec<CanFrame>,
    receive_cursor: usize,
    next_start: usize,
}

impl Default for SimulationBus {
    fn default() -> Self {
        Self {
            realm: Arc::new(()),
            receive: vec![VecDeque::new()],
            reports: VecDeque::new(),
            rules: Vec::new(),
            next_rule: 0,
            transmissions: Vec::new(),
            frames: Vec::new(),
            receive_cursor: 0,
            next_start: 0,
        }
    }
}

impl SimulationBus {
    pub(crate) fn realm(&self) -> &Arc<()> {
        &self.realm
    }

    pub(crate) fn access(&mut self) -> SimulationAccess<'_> {
        SimulationAccess { bus: self }
    }
    pub fn frames(&self) -> &[CanFrame] {
        &self.frames
    }

    pub fn transmissions(&self) -> &[SimulationTransmission] {
        &self.transmissions
    }

    pub fn clear_trace(&mut self) {
        self.transmissions.clear();
        self.frames.clear();
    }

    pub fn pending_receive_count(&self) -> usize {
        self.receive.iter().map(VecDeque::len).sum()
    }

    pub fn set_source_count(&mut self, sources: usize) -> Result<(), SimulationError> {
        if !(1..=64).contains(&sources) {
            return Err(SimulationError::SourceCount);
        }
        if sources < self.receive.len()
            && (self.receive[sources..]
                .iter()
                .any(|queue| !queue.is_empty())
                || self.rules.iter().any(|rule| rule.receive_source >= sources))
        {
            return Err(SimulationError::Source);
        }
        self.receive.resize_with(sources, VecDeque::new);
        self.receive_cursor %= sources;
        self.next_start %= sources;
        Ok(())
    }

    pub fn queue_frame(&mut self, frame: CanFrame) -> Result<(), SimulationError> {
        self.queue_attempts([frame.into()])
    }

    pub fn queue_received(&mut self, frame: ReceivedCanFrame) -> Result<(), SimulationError> {
        self.queue_attempts([SimulationReceive::Received(frame)])
    }

    pub fn queue_timed(&mut self, frame: TimedCanFrame) -> Result<(), SimulationError> {
        self.queue_attempts([SimulationReceive::Timed(frame)])
    }

    pub fn queue_frames(
        &mut self,
        frames: impl IntoIterator<Item = CanFrame>,
    ) -> Result<(), SimulationError> {
        self.queue_attempts(frames.into_iter().map(SimulationReceive::from))
    }

    pub fn queue_attempts(
        &mut self,
        attempts: impl IntoIterator<Item = SimulationReceive>,
    ) -> Result<(), SimulationError> {
        self.queue_to_source(0, attempts)
    }

    pub fn queue_to_source(
        &mut self,
        source: usize,
        attempts: impl IntoIterator<Item = SimulationReceive>,
    ) -> Result<(), SimulationError> {
        if source >= self.receive.len() {
            return Err(SimulationError::Source);
        }
        let remaining = MAX_SCRIPT_ITEMS.saturating_sub(self.pending_receive_count());
        let staged: Vec<_> = attempts.into_iter().take(remaining + 1).collect();
        if staged.len() > remaining {
            return Err(SimulationError::Capacity);
        }
        self.receive[source].extend(staged);
        Ok(())
    }

    /// Impossible-on-wire consumer cases, not reference evidence. Raw scripts
    /// should be preferred for decoder/transport tests.
    pub fn queue_feedback_report(&mut self, report: FeedbackReport) -> Result<(), SimulationError> {
        if report.observations.len() + report.transport_frames.len() > 64
            || report.raw_frames > 64
            || report.read_attempts > 256
        {
            return Err(SimulationError::ReportCapacity);
        }
        if self.reports.len() >= 128 {
            return Err(SimulationError::Capacity);
        }
        self.reports.push_back(report);
        Ok(())
    }

    pub fn add_tx_rule(&mut self, rule: TxRule) -> Result<RuleId, SimulationError> {
        self.add_tx_rule_to_source(0, rule)
    }

    pub fn add_tx_rule_to_source(
        &mut self,
        source: usize,
        rule: TxRule,
    ) -> Result<RuleId, SimulationError> {
        if source >= self.receive.len() {
            return Err(SimulationError::Source);
        }
        if self.rules.len() >= MAX_TX_RULES || rule.receive.len() > MAX_SCRIPT_ITEMS {
            return Err(SimulationError::Capacity);
        }
        if matches!(rule.occurrence, TxOccurrence::Nth(0)) {
            return Err(SimulationError::Occurrence);
        }
        let id = RuleId(self.next_rule);
        self.next_rule = self.next_rule.saturating_add(1);
        self.rules.push(InstalledRule {
            id,
            rule,
            matched: 0,
            triggered: 0,
            receive_source: source,
        });
        Ok(id)
    }

    pub fn rule_trigger_count(&self, id: RuleId) -> usize {
        self.rules
            .iter()
            .find(|rule| rule.id == id)
            .map_or(0, |rule| rule.triggered)
    }

    pub fn remove_tx_rule(&mut self, id: RuleId) {
        self.rules.retain(|rule| rule.id != id);
    }

    fn transmit(
        &mut self,
        address: Option<&MotorAddress>,
        frame: &CanFrame,
    ) -> Result<(), BusError> {
        if self.transmissions.len() >= MAX_TRACE_ITEMS {
            return Err(BusError::Send {
                message: SimulationError::Capacity.to_string(),
            });
        }
        let mut failure = None;
        let mut pending = self.pending_receive_count();
        for installed in &mut self.rules {
            if !installed.rule.matcher.matches(address, frame) {
                continue;
            }
            installed.matched = installed.matched.saturating_add(1);
            let triggered = match installed.rule.occurrence {
                TxOccurrence::Nth(n) => installed.matched == n,
                TxOccurrence::Every => true,
            };
            if !triggered {
                continue;
            }
            installed.triggered = installed.triggered.saturating_add(1);
            if pending + installed.rule.receive.len() > MAX_SCRIPT_ITEMS {
                failure.get_or_insert_with(|| SimulationError::Capacity.to_string());
            } else {
                pending += installed.rule.receive.len();
                self.receive[installed.receive_source]
                    .extend(installed.rule.receive.iter().cloned());
            }
            if let Some(error) = &installed.rule.send_error {
                failure.get_or_insert_with(|| error.clone());
            }
        }
        self.frames.push(frame.clone());
        self.transmissions.push(SimulationTransmission {
            address: address.cloned(),
            frame: frame.clone(),
            delivered: failure.is_none(),
        });
        if let Some(message) = failure {
            Err(BusError::Send { message })
        } else {
            Ok(())
        }
    }
}

/// Restricted script/trace access. No mutable transport reference or bus/state
/// conversion is exposed, so replacing and restoring the owner realm is impossible.
pub struct SimulationAccess<'a> {
    bus: &'a mut SimulationBus,
}

impl<'a> SimulationAccess<'a> {
    pub fn frames(self) -> &'a [CanFrame] {
        self.bus.frames()
    }

    pub fn transmissions(self) -> &'a [SimulationTransmission] {
        self.bus.transmissions()
    }

    pub fn clear_trace(&mut self) {
        self.bus.clear_trace();
    }

    pub fn pending_receive_count(&self) -> usize {
        self.bus.pending_receive_count()
    }

    pub fn set_source_count(&mut self, sources: usize) -> Result<(), SimulationError> {
        self.bus.set_source_count(sources)
    }

    pub fn queue_frame(&mut self, frame: CanFrame) -> Result<(), SimulationError> {
        self.bus.queue_frame(frame)
    }

    pub fn queue_received(&mut self, frame: ReceivedCanFrame) -> Result<(), SimulationError> {
        self.bus.queue_received(frame)
    }

    pub fn queue_timed(&mut self, frame: TimedCanFrame) -> Result<(), SimulationError> {
        self.bus.queue_timed(frame)
    }

    pub fn queue_frames(
        &mut self,
        frames: impl IntoIterator<Item = CanFrame>,
    ) -> Result<(), SimulationError> {
        self.bus.queue_frames(frames)
    }

    pub fn queue_attempts(
        &mut self,
        attempts: impl IntoIterator<Item = SimulationReceive>,
    ) -> Result<(), SimulationError> {
        self.bus.queue_attempts(attempts)
    }

    pub fn queue_to_source(
        &mut self,
        source: usize,
        attempts: impl IntoIterator<Item = SimulationReceive>,
    ) -> Result<(), SimulationError> {
        self.bus.queue_to_source(source, attempts)
    }

    pub fn queue_feedback_report(&mut self, report: FeedbackReport) -> Result<(), SimulationError> {
        self.bus.queue_feedback_report(report)
    }

    pub fn add_tx_rule(&mut self, rule: TxRule) -> Result<RuleId, SimulationError> {
        self.bus.add_tx_rule(rule)
    }

    pub fn add_tx_rule_to_source(
        &mut self,
        source: usize,
        rule: TxRule,
    ) -> Result<RuleId, SimulationError> {
        self.bus.add_tx_rule_to_source(source, rule)
    }

    pub fn rule_trigger_count(&self, id: RuleId) -> usize {
        self.bus.rule_trigger_count(id)
    }

    pub fn remove_tx_rule(&mut self, id: RuleId) {
        self.bus.remove_tx_rule(id);
    }

    /// Inspect a saturated unread suffix through the real bounded raw engine.
    pub fn drain_raw(&mut self) -> robstride::RawReceiveReport {
        self.bus.recv_raw_report(
            Duration::ZERO,
            Duration::ZERO,
            robstride::ReceiveLimits::default(),
        )
    }
}

impl CanBus for SimulationBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.transmit(None, frame)
    }

    fn send_frame_to(&mut self, address: &MotorAddress, frame: &CanFrame) -> Result<(), BusError> {
        self.transmit(Some(address), frame)
    }

    fn receive_source_count(&self) -> usize {
        self.receive.len()
    }

    fn begin_receive(&mut self) {
        self.receive_cursor = self.next_start;
        self.next_start = (self.next_start + 1) % self.receive.len();
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        let source = self.receive_cursor;
        self.receive_cursor = (self.receive_cursor + 1) % self.receive.len();
        match self.receive[source]
            .pop_front()
            .unwrap_or(SimulationReceive::Idle)
        {
            SimulationReceive::Received(received) => Ok(ReceiveAttempt::Frame(TimedCanFrame {
                received_at: Instant::now(),
                received,
            })),
            SimulationReceive::Timed(frame) => Ok(ReceiveAttempt::Frame(frame)),
            SimulationReceive::Idle => Ok(ReceiveAttempt::Idle),
            SimulationReceive::Interrupted => Ok(ReceiveAttempt::Interrupted),
            SimulationReceive::Error(message) => Err(BusError::Driver(message)),
        }
    }
}

impl MotorBus for SimulationBus {
    fn recv_feedback_report(
        &mut self,
        types: &HashMap<MotorAddress, MotorType>,
        budget: Duration,
        quiet: Duration,
    ) -> FeedbackReport {
        if let Some(report) = self.reports.pop_front() {
            report
        } else {
            // Decode through the ordinary default MotorBus implementation using
            // this closed raw transport, not a delegated or copied decoder.
            self.recv_feedback_report_with_limits(
                types,
                budget,
                quiet,
                robstride::ReceiveLimits::default(),
            )
        }
    }
}

impl Supervisor<SimulationBus> {
    /// Software admission/output coverage from a declared INITIAL virtual
    /// reference. Does not test acquisition, SetZero correlation or persistence.
    pub fn from_simulation(
        repo_root: impl AsRef<std::path::Path>,
        bus: SimulationBus,
        initial_reference: InitialVirtualReference,
    ) -> Result<Self, DavoutError> {
        let mut supervisor = Self::from_repo(repo_root, bus)?;
        supervisor.install_initial_virtual_reference(initial_reference)?;
        Ok(supervisor)
    }

    pub fn from_simulation_with_calibration_record_path(
        repo_root: impl AsRef<std::path::Path>,
        bus: SimulationBus,
        record_path: impl AsRef<std::path::Path>,
        initial_reference: InitialVirtualReference,
    ) -> Result<Self, DavoutError> {
        let mut supervisor =
            Self::from_repo_with_calibration_record_path(repo_root, bus, record_path)?;
        supervisor.install_initial_virtual_reference(initial_reference)?;
        Ok(supervisor)
    }

    fn install_initial_virtual_reference(
        &mut self,
        initial: InitialVirtualReference,
    ) -> Result<(), DavoutError> {
        let joints: std::collections::HashSet<String> = match initial {
            InitialVirtualReference::Unreferenced => return Ok(()),
            InitialVirtualReference::AllConfigured => self
                .motors
                .motors
                .iter()
                .map(|motor| motor.joint.clone())
                .collect(),
            InitialVirtualReference::Joints(joints) => {
                let mut unique = std::collections::HashSet::new();
                for joint in joints {
                    if !self.motors.motors.iter().any(|motor| motor.joint == joint) {
                        return Err(DavoutError::UnknownJoint { joint });
                    }
                    if !unique.insert(joint.clone()) {
                        return Err(DavoutError::Homing {
                            message: format!("duplicate INITIAL virtual reference joint {joint}"),
                        });
                    }
                }
                unique
            }
        };
        self.reference_authority = crate::reference::ReferenceAuthority::initial_virtual(
            Arc::clone(self.bus.realm()),
            joints,
            &self.motors,
            &self.homing_config,
            &self.control,
        );
        // Fixed private read-only comparison, assigned only for this concrete
        // specialization. Callers cannot supply a callback or backend marker.
        self.reference_realm_matches = Some(|bus, realm| Arc::ptr_eq(bus.realm(), realm));
        Ok(())
    }

    /// Finite scripts/trace access only. Cannot extract or replace the owner bus.
    pub fn bus_mut(&mut self) -> SimulationAccess<'_> {
        self.bus.access()
    }
}
