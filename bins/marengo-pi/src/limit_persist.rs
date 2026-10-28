//! Coalescing write-behind for control.yaml / motors+URDF (never on the 200 Hz tick).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use armee_proto::{ActionEvent, PersistStatus};
use chappe::Bus;
use marengo_config::{
    profile_content_revision, write_control_config_from, write_motors_control_and_urdf,
    ControlConfigFile, MotorsConfigFile,
};
use thiserror::Error;
use tracing::{info, warn};

#[cfg(test)]
#[path = "limit_persist_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "limit_persist_qualification_tests.rs"]
mod qualification_tests;

#[cfg(test)]
type PersistRequestObserver = Arc<dyn Fn(&PersistRequest) + Send + Sync>;

/// Observers/gates at the actual worker's I/O boundaries; never replace a write.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct PersistTestHooks {
    pub before_write: Option<PersistRequestObserver>,
    pub before_publish: Option<PersistRequestObserver>,
    pub on_worker_exit: Option<Arc<dyn Fn() + Send + Sync>>,
}

#[cfg(test)]
struct WorkerExitObserver(Option<Arc<dyn Fn() + Send + Sync>>);

#[cfg(test)]
impl Drop for WorkerExitObserver {
    fn drop(&mut self) {
        if let Some(observer) = &self.0 {
            observer();
        }
    }
}

pub(crate) use chappe::topics::TOPIC_AUDIT_ACTION;

static AUDIT_REVISION: AtomicU64 = AtomicU64::new(1);

pub(crate) fn publish_action_event(
    chappe: &Bus,
    event: &ActionEvent,
) -> Result<(), chappe::BusError> {
    chappe.publish(
        TOPIC_AUDIT_ACTION,
        "marengo-pi",
        "marengo.v1.ActionEvent",
        event,
    )
}

/// Next audit revision for ActionEvent rows (live ACK + write-behind).
pub(crate) fn next_audit_revision() -> u64 {
    AUDIT_REVISION.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Error)]
pub enum PersistError {
    #[error("persist queue unavailable: {0}")]
    Queue(String),
}

/// Coalescing background writer for config YAML (+ expand-only URDF when motors present).
pub struct ConfigPersistQueue {
    pending: Arc<Mutex<PersistSlot>>,
    changed: Arc<Condvar>,
    wake_tx: SyncSender<()>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    /// The root the worker writes the URDF under; CAS revisions must hash the same file.
    repo_root: PathBuf,
}

struct PersistSlot {
    request: Option<PersistRequest>,
    /// Coalesced limit patches replaced by `request`; each still gets its own
    /// completion event when `request` is written: Durable only when the
    /// written snapshot holds its bounds, otherwise superseded (Failed).
    carried: Vec<CarriedCompletion>,
    writing: bool,
    accepting: bool,
    worker_done: bool,
    worker_error: Option<String>,
    successful_writes: u64,
    failed_writes: u64,
    publication_failures: u64,
    coalesced_requests: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PersistDrainStatus {
    Complete,
    CompletedWithFailures,
    TimedOut,
    WorkerFailed,
}

/// Per-retained-request filesystem results and local publication outcomes.
/// A timeout leaves the worker running; it does not cancel its filesystem work.
/// Counters finalize after the local publication attempt returns. In-flight
/// includes writes already on disk whose matching completion has not returned;
/// it does not imply that the disk is unchanged. Coalesced requests are counted
/// separately. A coalesced limit patch gets the later write's completion when
/// that write holds its bounds, and a superseded (Failed) completion when a
/// newer limit patch replaced them; nothing else gets an invented one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PersistDrainReport {
    pub status: PersistDrainStatus,
    pub successful_writes: u64,
    pub failed_writes: u64,
    pub publication_failures: u64,
    pub pending_requests: usize,
    pub in_flight: bool,
    pub worker_terminated: bool,
    pub worker_error: Option<String>,
    pub coalesced_requests: u64,
}

impl PersistDrainReport {
    fn from_slot(slot: &PersistSlot, worker_terminated: bool) -> Self {
        let pending_requests = usize::from(slot.request.is_some());
        let status = if slot.worker_error.is_some() {
            PersistDrainStatus::WorkerFailed
        } else if !worker_terminated || pending_requests != 0 || slot.writing {
            PersistDrainStatus::TimedOut
        } else if slot.failed_writes != 0 || slot.publication_failures != 0 {
            PersistDrainStatus::CompletedWithFailures
        } else {
            PersistDrainStatus::Complete
        };
        Self {
            status,
            successful_writes: slot.successful_writes,
            failed_writes: slot.failed_writes,
            publication_failures: slot.publication_failures,
            pending_requests,
            in_flight: slot.writing,
            worker_terminated,
            worker_error: slot.worker_error.clone(),
            coalesced_requests: slot.coalesced_requests,
        }
    }

    pub fn is_idle(&self) -> bool {
        self.pending_requests == 0 && !self.in_flight
    }
}

/// Publish worker completion even during unwinding, retaining unfinished work.
struct WorkerCompletion {
    pending: Arc<Mutex<PersistSlot>>,
    changed: Arc<Condvar>,
    completed_normally: bool,
}

impl Drop for WorkerCompletion {
    fn drop(&mut self) {
        let mut slot = self.pending.lock().unwrap_or_else(|error| {
            let mut slot = error.into_inner();
            slot.worker_error
                .get_or_insert_with(|| "persist state lock poisoned".into());
            slot
        });
        slot.accepting = false;
        slot.worker_done = true;
        if !self.completed_normally {
            slot.worker_error
                .get_or_insert_with(|| "persist worker exited unexpectedly".into());
        }
        self.changed.notify_all();
    }
}

pub(crate) struct PersistRequest {
    pub config_dir: PathBuf,
    /// When set, write motors.yaml + control.yaml + expand-only URDF; otherwise control-only.
    pub motors: Option<MotorsConfigFile>,
    pub control: ControlConfigFile,
    pub timestamp_ms: u64,
    pub session_id: String,
    pub operator_id: String,
    pub joint: String,
    pub param: String,
}
/// Identity and bounds of a coalesced limit patch, for its completion event.
struct CarriedCompletion {
    timestamp_ms: u64,
    session_id: String,
    operator_id: String,
    joint: String,
    bounds: CarriedBounds,
}

/// A limit patch's joint entries as its own snapshot held them.
#[derive(Debug, Clone, PartialEq)]
struct CarriedBounds {
    position_lower_rad: f64,
    position_upper_rad: f64,
    torque_limit_nm: f64,
    position_soft_lower_rad: Option<f64>,
    position_soft_upper_rad: Option<f64>,
    velocity_max_rad_s: Option<f64>,
}

impl CarriedBounds {
    /// True when `request` would write these bounds for `joint`. An unset
    /// optional field in the patch's own snapshot constrains nothing.
    fn written_by(&self, request: &PersistRequest, joint: &str) -> bool {
        let Some(motor) = request
            .motors
            .as_ref()
            .and_then(|motors| motors.motors.iter().find(|motor| motor.joint == joint))
        else {
            return false;
        };
        let Some(control) = request.control.control.joints.get(joint) else {
            return false;
        };
        let held = |patch: Option<f64>, written: Option<f64>| patch.is_none() || patch == written;
        motor.bench.position_lower_rad == self.position_lower_rad
            && motor.bench.position_upper_rad == self.position_upper_rad
            && motor.bench.torque_limit_nm == self.torque_limit_nm
            && held(
                self.position_soft_lower_rad,
                control.position_soft_lower_rad,
            )
            && held(
                self.position_soft_upper_rad,
                control.position_soft_upper_rad,
            )
            && held(self.velocity_max_rad_s, control.velocity_max_rad_s)
    }
}

/// Coalesce the pending limit patch `pending` into the later request `latest`.
///
/// Every limit patch request carries the full live motors and control
/// snapshot taken after its apply, so a later limit patch already holds every
/// earlier patch on other joints, and newer bounds on the same joint; it is
/// left untouched (newer wins). Only a control-only save, which writes no
/// motors, gets the pending snapshot and bounds. Either way the pending patch
/// keeps its completion, which the worker reports Durable only when the
/// written snapshot holds its bounds.
fn carry_pending_limit_patch(
    pending: &PersistRequest,
    latest: &mut PersistRequest,
) -> Result<CarriedCompletion, PersistError> {
    if pending.config_dir != latest.config_dir {
        return Err(PersistError::Queue(
            "cannot coalesce limit patches across config directories".into(),
        ));
    }
    let Some(previous_motors) = pending.motors.as_ref() else {
        return Err(PersistError::Queue(
            "pending limit patch has no motors snapshot".into(),
        ));
    };
    let previous_motor = previous_motors
        .motors
        .iter()
        .find(|motor| motor.joint == pending.joint)
        .ok_or_else(|| PersistError::Queue("pending limit patch motor is missing".into()))?;
    let previous_control = pending
        .control
        .control
        .joints
        .get(&pending.joint)
        .ok_or_else(|| PersistError::Queue("pending limit patch control is missing".into()))?;
    let carried = CarriedCompletion {
        timestamp_ms: pending.timestamp_ms,
        session_id: pending.session_id.clone(),
        operator_id: pending.operator_id.clone(),
        joint: pending.joint.clone(),
        bounds: CarriedBounds {
            position_lower_rad: previous_motor.bench.position_lower_rad,
            position_upper_rad: previous_motor.bench.position_upper_rad,
            torque_limit_nm: previous_motor.bench.torque_limit_nm,
            position_soft_lower_rad: previous_control.position_soft_lower_rad,
            position_soft_upper_rad: previous_control.position_soft_upper_rad,
            velocity_max_rad_s: previous_control.velocity_max_rad_s,
        },
    };
    if latest.motors.is_some() {
        return Ok(carried);
    }
    let patch = marengo_config::LimitPatch {
        joint: pending.joint.clone(),
        position_lower_rad: previous_motor.bench.position_lower_rad,
        position_upper_rad: previous_motor.bench.position_upper_rad,
        torque_limit_nm: None,
        position_soft_lower_rad: previous_control.position_soft_lower_rad,
        position_soft_upper_rad: previous_control.position_soft_upper_rad,
        velocity_max_rad_s: previous_control.velocity_max_rad_s,
    };
    let control = latest
        .control
        .control
        .joints
        .get_mut(&pending.joint)
        .ok_or_else(|| PersistError::Queue("latest control snapshot lacks patched joint".into()))?;
    let mut patched_control = control.clone();
    marengo_config::apply_limit_patch_to_control(&mut patched_control, &patch)
        .map_err(|error| PersistError::Queue(error.to_string()))?;
    *control = patched_control;
    latest.motors = Some(previous_motors.clone());
    Ok(carried)
}

impl ConfigPersistQueue {
    /// Spawn a worker that serializes YAML writes; latest enqueued draft wins.
    ///
    /// `repo_root` is resolved once at boot (install root with `assets/urdf/`).
    pub fn spawn(chappe: Arc<Bus>, repo_root: PathBuf) -> Self {
        Self::spawn_inner(
            chappe,
            repo_root,
            #[cfg(test)]
            PersistTestHooks::default(),
        )
    }

    /// Test constructor with worker hooks (before_write gate, on_worker_exit).
    #[cfg(test)]
    pub(crate) fn spawn_with_test_hooks(
        chappe: Arc<Bus>,
        repo_root: PathBuf,
        hooks: PersistTestHooks,
    ) -> Self {
        Self::spawn_inner(chappe, repo_root, hooks)
    }

    fn spawn_inner(
        chappe: Arc<Bus>,
        repo_root: PathBuf,
        #[cfg(test)] hooks: PersistTestHooks,
    ) -> Self {
        let (wake_tx, wake_rx) = sync_channel::<()>(1);
        let pending = Arc::new(Mutex::new(PersistSlot {
            request: None,
            carried: Vec::new(),
            writing: false,
            accepting: true,
            worker_done: false,
            worker_error: None,
            successful_writes: 0,
            failed_writes: 0,
            publication_failures: 0,
            coalesced_requests: 0,
        }));
        let changed = Arc::new(Condvar::new());
        let pending_worker = Arc::clone(&pending);
        let changed_worker = Arc::clone(&changed);
        let worker_root = repo_root.clone();
        let worker = thread::spawn(move || {
            persist_worker(
                pending_worker,
                changed_worker,
                wake_rx,
                chappe,
                worker_root,
                #[cfg(test)]
                hooks,
            )
        });
        Self {
            pending,
            changed,
            wake_tx,
            worker: Mutex::new(Some(worker)),
            repo_root,
        }
    }

    /// The root the worker resolves the URDF under. Revision checks pass it to
    /// `profile_content_revision` so the CAS covers the URDF this queue writes.
    pub(crate) fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// Queue a durable write. Replaces any not-yet-started request (coalesce).
    pub(crate) fn enqueue(&self, request: PersistRequest) -> Result<(), PersistError> {
        let mut slot = self
            .pending
            .lock()
            .map_err(|_| PersistError::Queue("lock poisoned".into()))?;
        if !slot.accepting || slot.worker_done || slot.worker_error.is_some() {
            return Err(PersistError::Queue("queue admission closed".into()));
        }
        match self.wake_tx.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => {
                let mut latest = request;
                if let Some(mut pending) = slot.request.take() {
                    if pending.param == "limit_patch" {
                        match carry_pending_limit_patch(&pending, &mut latest) {
                            // The caller's completion rides on `latest` (its
                            // gateway waits on this session id).
                            Ok(carried) => slot.carried.push(carried),
                            Err(error) => {
                                slot.request = Some(pending);
                                return Err(error);
                            }
                        }
                    } else if latest.motors.is_none() {
                        // A pending control save may itself carry earlier limit
                        // bounds; a later control-only save must not drop them.
                        latest.motors = pending.motors.take();
                    }
                    slot.coalesced_requests = slot.coalesced_requests.saturating_add(1);
                }
                slot.request = Some(latest);
                self.changed.notify_all();
                Ok(())
            }
            Err(TrySendError::Disconnected(())) => {
                slot.accepting = false;
                slot.worker_error
                    .get_or_insert_with(|| "persist worker disconnected".into());
                self.changed.notify_all();
                Err(PersistError::Queue("worker disconnected".into()))
            }
        }
    }

    /// Close admission, drain retained work and observe actual thread termination.
    /// Never joins a worker that has not finished; unfinished I/O remains explicit.
    pub(crate) fn close_and_drain(&self, timeout: Duration) -> PersistDrainReport {
        self.close_admission();
        let start = Instant::now();
        let mut slot = self.pending.lock().unwrap_or_else(|error| {
            let mut slot = error.into_inner();
            slot.worker_error
                .get_or_insert_with(|| "persist state lock poisoned".into());
            slot
        });
        slot.accepting = false;
        // Nonblocking wake under the same admission lock: a closing queue cannot
        // race a later successful enqueue or replace its retained request.
        let _ = self.wake_tx.try_send(());
        self.changed.notify_all();
        loop {
            let terminated = self.join_finished_worker(&mut slot);
            let remaining = timeout.saturating_sub(start.elapsed());
            if terminated || remaining.is_zero() {
                return PersistDrainReport::from_slot(&slot, terminated);
            }
            // The completion guard runs just before thread return. Wait briefly
            // for JoinHandle::is_finished rather than equating the guard with exit.
            let wait = if slot.worker_done {
                remaining.min(Duration::from_millis(1))
            } else {
                remaining
            };
            slot = match self.changed.wait_timeout(slot, wait) {
                Ok((slot, _)) => slot,
                Err(error) => {
                    let (mut slot, _) = error.into_inner();
                    slot.worker_error
                        .get_or_insert_with(|| "persist state lock poisoned".into());
                    slot
                }
            };
        }
    }

    /// Close without waiting, so composition can close every writer first.
    pub(crate) fn close_admission(&self) {
        let mut slot = self.pending.lock().unwrap_or_else(|error| {
            let mut slot = error.into_inner();
            slot.worker_error
                .get_or_insert_with(|| "persist state lock poisoned".into());
            slot
        });
        slot.accepting = false;
        let _ = self.wake_tx.try_send(());
        self.changed.notify_all();
    }

    fn join_finished_worker(&self, slot: &mut PersistSlot) -> bool {
        let Ok(mut worker) = self.worker.lock() else {
            slot.worker_error
                .get_or_insert_with(|| "persist worker handle lock poisoned".into());
            return false;
        };
        let Some(handle) = worker.as_ref() else {
            return true;
        };
        if !handle.is_finished() {
            return false;
        }
        if let Some(handle) = worker.take() {
            if handle.join().is_err() {
                slot.worker_error
                    .get_or_insert_with(|| "persist worker panicked".into());
            }
        }
        true
    }

    /// True while a write is queued or in flight (restart must drain first).
    #[cfg(test)]
    pub fn is_busy(&self) -> bool {
        self.pending
            .lock()
            .map(|slot| slot.request.is_some() || slot.writing)
            .unwrap_or(true)
    }

    /// Block until the persist queue is idle or `timeout` elapses.
    #[cfg(test)]
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if !self.is_busy() {
                return true;
            }
            thread::sleep(Duration::from_millis(5));
        }
        false
    }
}

impl Drop for ConfigPersistQueue {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.pending.lock() {
            slot.accepting = false;
        }
        let _ = self.wake_tx.try_send(());
        self.changed.notify_all();
    }
}

/// Audit `action` for async write-behind (never `"limit_patch"`, which is the live ACK).
fn persist_audit_action(param: &str) -> &'static str {
    if param == "limit_patch" {
        "limit_patch_persist"
    } else {
        "config_persist"
    }
}

fn persist_worker(
    pending: Arc<Mutex<PersistSlot>>,
    changed: Arc<Condvar>,
    wake_rx: Receiver<()>,
    chappe: Arc<Bus>,
    repo_root: PathBuf,
    #[cfg(test)] hooks: PersistTestHooks,
) {
    #[cfg(test)]
    let _exit_observer = WorkerExitObserver(hooks.on_worker_exit.clone());
    let mut completion = WorkerCompletion {
        pending: Arc::clone(&pending),
        changed: Arc::clone(&changed),
        completed_normally: false,
    };
    loop {
        {
            let Ok(slot) = pending.lock() else {
                return;
            };
            if !slot.accepting && slot.request.is_none() {
                break;
            }
        }
        match wake_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(()) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                let Ok(mut slot) = pending.lock() else {
                    return;
                };
                slot.accepting = false;
            }
        }
        while wake_rx.try_recv().is_ok() {}

        loop {
            let (request, carried) = {
                let Ok(mut slot) = pending.lock() else {
                    return;
                };
                let Some(req) = slot.request.take() else {
                    break;
                };
                slot.writing = true;
                (req, std::mem::take(&mut slot.carried))
            };

            #[cfg(test)]
            if let Some(observer) = &hooks.before_write {
                observer(&request);
            }

            let write_result = match &request.motors {
                Some(motors) => write_motors_control_and_urdf(
                    &repo_root,
                    &request.config_dir,
                    motors,
                    &request.control,
                ),
                None => write_control_config_from(&request.config_dir, &request.control),
            };
            #[cfg(test)]
            if let Some(observer) = &hooks.before_publish {
                observer(&request);
            }

            let write_succeeded = write_result.is_ok();
            // A coalesced patch shares the write's outcome only when the write
            // holds its bounds; a newer patch on its joint supersedes it, and
            // its caller must not be told (or sync) bounds that are not on disk.
            let carried: Vec<(CarriedCompletion, bool)> = carried
                .into_iter()
                .map(|carried| {
                    let written = carried.bounds.written_by(&request, &carried.joint);
                    (carried, written)
                })
                .collect();
            let carried_event = |(carried, written): (CarriedCompletion, bool),
                                 template: &ActionEvent| {
                let mut event = ActionEvent {
                    timestamp_ms: carried.timestamp_ms,
                    session_id: carried.session_id,
                    operator_id: carried.operator_id,
                    joint: carried.joint,
                    action: persist_audit_action("limit_patch").to_string(),
                    revision: next_audit_revision(),
                    ..template.clone()
                };
                if !written && template.accepted {
                    event.accepted = false;
                    event.reject_reason = format!(
                        "superseded: a later limit patch for {} was written instead of these bounds",
                        event.joint
                    );
                    event.persist_status = PersistStatus::Failed as i32;
                    event.config_revision = String::new();
                }
                event
            };
            let publication_result = match write_result {
                Ok(()) => {
                    info!(
                        joint = %request.joint,
                        param = %request.param,
                        session = %request.session_id,
                        persist = true,
                        "config persist write succeeded"
                    );
                    let config_revision = profile_content_revision(&repo_root, &request.config_dir)
                        .unwrap_or_default();
                    let event = ActionEvent {
                        timestamp_ms: request.timestamp_ms,
                        session_id: request.session_id,
                        operator_id: request.operator_id,
                        joint: request.joint,
                        // Write-behind uses a distinct action so gateway live-wait
                        // (action == "limit_patch") cannot treat Durable/Failed as the apply ACK.
                        action: persist_audit_action(&request.param).to_string(),
                        revision: next_audit_revision(),
                        accepted: true,
                        reject_reason: "disk write completed".to_string(),
                        persist_status: PersistStatus::Durable as i32,
                        config_revision,
                    };
                    let mut published = Ok(());
                    for carried in carried {
                        let carried = carried_event(carried, &event);
                        published = published.and(publish_action_event(&chappe, &carried));
                    }
                    published.and(publish_action_event(&chappe, &event))
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        joint = %request.joint,
                        param = %request.param,
                        session = %request.session_id,
                        persist = true,
                        "config persist write FAILED; live config may differ from disk"
                    );
                    let event = ActionEvent {
                        timestamp_ms: request.timestamp_ms,
                        session_id: request.session_id,
                        operator_id: request.operator_id,
                        joint: request.joint,
                        action: persist_audit_action(&request.param).to_string(),
                        revision: next_audit_revision(),
                        accepted: false,
                        reject_reason: format!("async config write failed after live apply: {e}"),
                        persist_status: PersistStatus::Failed as i32,
                        config_revision: String::new(),
                    };
                    let mut published = Ok(());
                    for carried in carried {
                        let carried = carried_event(carried, &event);
                        published = published.and(publish_action_event(&chappe, &carried));
                    }
                    published.and(publish_action_event(&chappe, &event))
                }
            };
            if let Err(error) = &publication_result {
                warn!(error = %error, "config persist completion publication failed");
            }
            let Ok(mut slot) = pending.lock() else {
                return;
            };
            if write_succeeded {
                slot.successful_writes = slot.successful_writes.saturating_add(1);
            } else {
                slot.failed_writes = slot.failed_writes.saturating_add(1);
            }
            if publication_result.is_err() {
                slot.publication_failures = slot.publication_failures.saturating_add(1);
            }
            slot.writing = false;
            changed.notify_all();
        }
    }
    completion.completed_normally = true;
}
