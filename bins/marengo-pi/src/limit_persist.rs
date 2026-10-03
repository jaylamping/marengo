//! Coalescing write-behind for control.yaml / motors+URDF (never on the 200 Hz tick).

use std::path::PathBuf;
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
}

struct PersistSlot {
    request: Option<PersistRequest>,
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
/// separately and receive no invented completion event.
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
fn carry_pending_limit_patch(
    pending: &PersistRequest,
    latest: &mut PersistRequest,
) -> Result<(), PersistError> {
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
    let patch = marengo_config::LimitPatch {
        joint: pending.joint.clone(),
        position_lower_rad: previous_motor.bench.position_lower_rad,
        position_upper_rad: previous_motor.bench.position_upper_rad,
        torque_limit_nm: None,
        position_soft_lower_rad: previous_control.position_soft_lower_rad,
        position_soft_upper_rad: previous_control.position_soft_upper_rad,
        velocity_max_rad_s: previous_control.velocity_max_rad_s,
    };
    let motors = latest.motors.get_or_insert_with(|| previous_motors.clone());
    let motor = motors
        .motors
        .iter_mut()
        .find(|motor| motor.joint == pending.joint)
        .ok_or_else(|| PersistError::Queue("latest motors snapshot lacks patched joint".into()))?;
    marengo_config::apply_limit_patch_to_motor(motor, &patch)
        .map_err(|error| PersistError::Queue(error.to_string()))?;
    let control = latest
        .control
        .control
        .joints
        .get_mut(&pending.joint)
        .ok_or_else(|| PersistError::Queue("latest control snapshot lacks patched joint".into()))?;
    marengo_config::apply_limit_patch_to_control(control, &patch)
        .map_err(|error| PersistError::Queue(error.to_string()))
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
        let worker = thread::spawn(move || {
            persist_worker(
                pending_worker,
                changed_worker,
                wake_rx,
                chappe,
                repo_root,
                #[cfg(test)]
                hooks,
            )
        });
        Self {
            pending,
            changed,
            wake_tx,
            worker: Mutex::new(Some(worker)),
        }
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
                if let Some(pending) = slot.request.take() {
                    if pending.param == "limit_patch" {
                        if let Err(error) = carry_pending_limit_patch(&pending, &mut latest) {
                            slot.request = Some(pending);
                            return Err(error);
                        }
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
            let request = {
                let Ok(mut slot) = pending.lock() else {
                    return;
                };
                let Some(req) = slot.request.take() else {
                    break;
                };
                slot.writing = true;
                req
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
            let publication_result = match write_result {
                Ok(()) => {
                    info!(
                        joint = %request.joint,
                        param = %request.param,
                        session = %request.session_id,
                        persist = true,
                        "config persist write succeeded"
                    );
                    let config_revision =
                        profile_content_revision(&request.config_dir).unwrap_or_default();
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
                    publish_action_event(&chappe, &event)
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
                    publish_action_event(&chappe, &event)
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
