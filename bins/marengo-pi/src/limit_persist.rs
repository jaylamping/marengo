//! Coalescing write-behind for control.yaml / motors+URDF (never on the 200 Hz tick).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(test)]
use std::time::Instant;

use armee_proto::{ActionEvent, PersistStatus};
use chappe::Bus;
use marengo_config::{
    profile_content_revision, write_control_config_from, write_motors_control_and_urdf,
    ControlConfigFile, MotorsConfigFile,
};
use thiserror::Error;
use tracing::{info, warn};

const TOPIC_AUDIT_ACTION: &str = "robot/audit/action";

static AUDIT_REVISION: AtomicU64 = AtomicU64::new(1);

fn publish_action_event(chappe: &Arc<Bus>, event: &ActionEvent) -> Result<(), chappe::BusError> {
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
    wake_tx: SyncSender<()>,
}

struct PersistSlot {
    request: Option<PersistRequest>,
    writing: bool,
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

impl ConfigPersistQueue {
    /// Spawn a worker that serializes YAML writes; latest enqueued draft wins.
    ///
    /// `repo_root` is resolved once at boot (install root with `assets/urdf/`).
    pub fn spawn(chappe: Arc<Bus>, shutdown: Arc<AtomicBool>, repo_root: PathBuf) -> Self {
        let (wake_tx, wake_rx) = sync_channel::<()>(1);
        let pending = Arc::new(Mutex::new(PersistSlot {
            request: None,
            writing: false,
        }));
        let pending_worker = Arc::clone(&pending);
        thread::spawn(move || persist_worker(pending_worker, wake_rx, chappe, shutdown, repo_root));
        Self { pending, wake_tx }
    }

    /// Queue a durable write. Replaces any not-yet-started request (coalesce).
    pub(crate) fn enqueue(&self, request: PersistRequest) -> Result<(), PersistError> {
        {
            let mut slot = self
                .pending
                .lock()
                .map_err(|_| PersistError::Queue("lock poisoned".into()))?;
            slot.request = Some(request);
        }
        match self.wake_tx.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => Ok(()),
            Err(TrySendError::Disconnected(())) => {
                Err(PersistError::Queue("worker disconnected".into()))
            }
        }
    }

    /// True while a write is queued or in flight (restart must drain first).
    pub fn is_busy(&self) -> bool {
        self.pending
            .lock()
            .map(|slot| slot.request.is_some() || slot.writing)
            .unwrap_or(true)
    }

    /// Block until the persist queue is idle or `timeout` elapses.
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        #[cfg(test)]
        let start = Instant::now();
        #[cfg(not(test))]
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if !self.is_busy() {
                return true;
            }
            thread::sleep(Duration::from_millis(5));
        }
        false
    }

    #[cfg(test)]
    pub(crate) fn wait_idle_for_test(&self, timeout: Duration) -> bool {
        self.wait_idle(timeout)
    }

    #[cfg(test)]
    pub(crate) fn pending_count_for_test(&self) -> usize {
        self.pending
            .lock()
            .map(|s| usize::from(s.request.is_some()))
            .unwrap_or(0)
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
    wake_rx: Receiver<()>,
    chappe: Arc<Bus>,
    shutdown: Arc<AtomicBool>,
    repo_root: PathBuf,
) {
    while !shutdown.load(Ordering::Relaxed) {
        match wake_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(()) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
        while wake_rx.try_recv().is_ok() {}

        loop {
            if shutdown.load(Ordering::Relaxed) {
                return;
            }
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

            let write_result = match &request.motors {
                Some(motors) => write_motors_control_and_urdf(
                    &repo_root,
                    &request.config_dir,
                    motors,
                    &request.control,
                ),
                None => write_control_config_from(&request.config_dir, &request.control),
            };
            if let Ok(mut slot) = pending.lock() {
                slot.writing = false;
            }

            match write_result {
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
                    let _ = publish_action_event(&chappe, &event);
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
                    let _ = publish_action_event(&chappe, &event);
                }
            }
        }
    }
}
