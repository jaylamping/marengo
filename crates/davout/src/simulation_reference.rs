//! Private virtual device continuity and exact raw-pop correlation.
//!
//! No token, callback, transport or output permission is exported by this module.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use robstride::{BusError, CanFrame, MotorAddress, ReceivedCanFrame, TimedCanFrame};

use super::{RuleId, SimulationBus, SimulationError, TxOccurrence, MAX_TX_RULES};
use crate::reference_transaction::{ReferenceBackend, ReferenceCorrelation};

/// Finite virtual reply provenance; never physical firmware evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceProofMode {
    CurrentSetZero,
    PreviousDeviceEpoch,
    PreviousTransaction,
    ForeignRealm,
}

/// One finite raw reply triggered only by the active target's actual SetZero.
/// Failed writes can still enqueue the effect, but cannot qualify acquisition.
#[derive(Debug, Clone)]
pub struct ReferenceReplyRule {
    pub address: MotorAddress,
    pub occurrence: TxOccurrence,
    pub frame: ReceivedCanFrame,
    pub proof: ReferenceProofMode,
    pub send_error: Option<String>,
}

#[derive(Clone)]
struct Context {
    owner: Arc<()>,
    transaction: u64,
    address: MotorAddress,
}

pub(super) struct ZeroAttempt {
    current: Context,
    previous: Option<Context>,
    previous_epoch: u64,
    device_epoch: u64,
}

/// Metadata stays with its own queued raw frame until that exact frame is popped.
#[derive(Clone)]
pub(super) struct QueuedProof {
    pub(super) rule: RuleId,
    context: Context,
    realm: Arc<()>,
    device_epoch: u64,
}

impl QueuedProof {
    pub(super) fn into_correlation(
        self,
        frame: &TimedCanFrame,
        order: usize,
    ) -> ReferenceCorrelation {
        ReferenceCorrelation {
            owner: self.context.owner,
            realm: self.realm,
            transaction: self.context.transaction,
            device_epoch: self.device_epoch,
            address: self.context.address,
            order,
            can_id: frame.received.frame.id,
            received_at: frame.received_at,
        }
    }
}

pub(super) struct ReferenceState {
    realm: Arc<()>,
    foreign_realm: Arc<()>,
    clock: Duration,
    active: Option<Context>,
    previous: Option<Context>,
    epochs: HashMap<MotorAddress, u64>,
    prepared: bool,
    report_order: Option<usize>,
    proofs: Vec<ReferenceCorrelation>,
}

impl ReferenceState {
    pub(super) fn new(realm: Arc<()>) -> Self {
        Self {
            realm,
            foreign_realm: Arc::new(()),
            clock: Duration::ZERO,
            active: None,
            previous: None,
            epochs: HashMap::new(),
            prepared: false,
            report_order: None,
            proofs: Vec::new(),
        }
    }

    fn begin(
        &mut self,
        owner: Arc<()>,
        transaction: u64,
        address: MotorAddress,
    ) -> Result<(), SimulationError> {
        if self.active.is_some()
            || self.previous.as_ref().is_some_and(|previous| {
                !Arc::ptr_eq(&previous.owner, &owner) || transaction <= previous.transaction
            })
        {
            return Err(SimulationError::ReferenceContext);
        }
        if !self.epochs.contains_key(&address) && self.epochs.len() >= MAX_TX_RULES {
            return Err(SimulationError::Capacity);
        }
        self.epochs.entry(address.clone()).or_insert(0);
        self.active = Some(Context {
            owner,
            transaction,
            address,
        });
        self.clear_report();
        Ok(())
    }

    fn end(&mut self) {
        if let Some(context) = self.active.take() {
            self.previous = Some(context);
        }
        self.clear_report();
        // Queued raw replies are deliberately retained by SimulationBus.
    }

    pub(super) fn has_previous(&self) -> bool {
        self.previous.is_some()
    }

    pub(super) fn elapse(&mut self, elapsed: Duration) -> Result<(), SimulationError> {
        self.clock = self
            .clock
            .checked_add(elapsed)
            .ok_or(SimulationError::ReferenceClockOverflow)?;
        Ok(())
    }

    pub(super) fn zero_attempt(
        &mut self,
        address: Option<&MotorAddress>,
        frame: &CanFrame,
    ) -> Result<Option<ZeroAttempt>, SimulationError> {
        let Some(active) = self.active.as_ref() else {
            return Ok(None);
        };
        let (id, data) = robstride::encode_default_set_zero_position(active.address.device_id);
        if address != Some(&active.address)
            || !frame.extended
            || frame.id != id
            || frame.data != data
        {
            return Ok(None);
        }
        let Some(epoch) = self.epochs.get_mut(&active.address) else {
            return Err(SimulationError::ReferenceContext);
        };
        let previous_epoch = *epoch;
        *epoch = (*epoch)
            .checked_add(1)
            .ok_or(SimulationError::ReferenceCounterExhausted)?;
        Ok(Some(ZeroAttempt {
            current: active.clone(),
            previous: self.previous.clone(),
            previous_epoch,
            device_epoch: *epoch,
        }))
    }

    pub(super) fn proof_for(
        &self,
        attempt: &ZeroAttempt,
        rule: RuleId,
        mode: ReferenceProofMode,
    ) -> Result<QueuedProof, SimulationError> {
        let context = if mode == ReferenceProofMode::PreviousTransaction {
            attempt
                .previous
                .clone()
                .ok_or(SimulationError::PreviousTransaction)?
        } else {
            attempt.current.clone()
        };
        Ok(QueuedProof {
            rule,
            context,
            realm: Arc::clone(if mode == ReferenceProofMode::ForeignRealm {
                &self.foreign_realm
            } else {
                &self.realm
            }),
            device_epoch: if mode == ReferenceProofMode::PreviousDeviceEpoch {
                attempt.previous_epoch
            } else {
                attempt.device_epoch
            },
        })
    }

    pub(super) fn clear_report(&mut self) {
        self.prepared = false;
        self.report_order = None;
        self.proofs.clear();
    }

    fn prepare_report(&mut self) {
        self.clear_report();
        self.prepared = true;
    }

    pub(super) fn begin_report(&mut self) {
        self.proofs.clear();
        self.report_order = self.prepared.then_some(0);
        self.prepared = false;
    }

    pub(super) fn popped(&mut self, frame: &TimedCanFrame, proof: Option<QueuedProof>) {
        let Some(order) = self.report_order else {
            return;
        };
        self.report_order = order.checked_add(1);
        if order < robstride::MAX_RX_FRAMES_PER_POLL {
            if let Some(proof) = proof {
                self.proofs.push(proof.into_correlation(frame, order));
            }
        }
    }

    fn take_proofs(&mut self) -> Vec<ReferenceCorrelation> {
        self.prepared = false;
        self.report_order = None;
        std::mem::take(&mut self.proofs)
    }

    fn device_epoch(&self) -> Option<u64> {
        self.active
            .as_ref()
            .and_then(|context| self.epochs.get(&context.address))
            .copied()
    }

    fn current_device_epoch(&self, address: &MotorAddress) -> Option<u64> {
        self.epochs.get(address).copied()
    }
}

impl SimulationBus {
    pub(super) fn reference_backend(&self) -> ReferenceBackend<Self> {
        ReferenceBackend {
            realm: Arc::clone(&self.realm),
            matches: |bus, realm| Arc::ptr_eq(bus.realm(), realm),
            now: |bus| bus.reference.clock,
            begin: |bus, owner, transaction, address| {
                bus.reference
                    .begin(owner, transaction, address)
                    .map_err(|error| BusError::Driver(error.to_string()))
            },
            end: |bus| bus.reference.end(),
            prepare_report: |bus| bus.reference.prepare_report(),
            take_proofs: |bus| bus.reference.take_proofs(),
            device_epoch: |bus| bus.reference.device_epoch(),
            current_device_epoch: |bus, address| bus.reference.current_device_epoch(address),
        }
    }
}
