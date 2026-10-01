use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use armee_proto::prost::Message;
use armee_proto::{
    ActionEvent, ActuatorLimitSnapshot, Heartbeat, HostMetrics, ImuSample, PersistStatus,
    RobotState, SafetyState,
};
use chappe::ipc::IpcListener;
use chappe::Bus;
use marengo_config::CommandJointAllowlist;
use tokio::sync::broadcast;

use crate::logs::{decode_log_payload, LogServices as LogSvc};
use crate::ratelimit::RateLimiter;

pub const TOPIC_STATE: &str = "robot/state";
pub const TOPIC_RUNTIME_CONNECTION: &str = "gateway/runtime_connection";
pub const TOPIC_SAFETY: &str = "robot/safety";
pub const TOPIC_HEARTBEAT: &str = "robot/heartbeat";
pub const TOPIC_IMU_TORSO: &str = "sensors/imu/torso";
pub const TOPIC_LOGS: &str = "logs/structured";
pub const TOPIC_HOST_METRICS_PI: &str = "host/metrics/pi";
pub const TOPIC_HOST_METRICS_JETSON: &str = "host/metrics/jetson";
pub const TOPIC_TESTING_MIT_COMMAND_BATCH: &str = "robot/testing/mit_command_batch";
pub const TOPIC_TESTING_TELEMETRY: &str = "robot/testing/telemetry";
pub const TOPIC_ACTUATOR_LIMITS: &str = "robot/actuator/limits";
pub const TOPIC_AUDIT_TUNING: &str = "robot/audit/tuning";
pub const TOPIC_AUDIT_ACTION: &str = "robot/audit/action";

/// Publish-only command topic (not in [`ALLOWED_TOPICS`]).
pub const TOPIC_ACTUATOR_COMMAND: &str = "robot/actuator/command";

pub const ALLOWED_TOPICS: &[&str] = &[
    TOPIC_RUNTIME_CONNECTION,
    TOPIC_STATE,
    TOPIC_SAFETY,
    TOPIC_HEARTBEAT,
    TOPIC_IMU_TORSO,
    TOPIC_LOGS,
    TOPIC_HOST_METRICS_PI,
    TOPIC_HOST_METRICS_JETSON,
    TOPIC_TESTING_MIT_COMMAND_BATCH,
    TOPIC_TESTING_TELEMETRY,
    TOPIC_ACTUATOR_LIMITS,
    TOPIC_AUDIT_TUNING,
    TOPIC_AUDIT_ACTION,
];

const ENVELOPE_BROADCAST_CAPACITY: usize = 4096;

#[derive(Default, Clone)]
pub struct Snapshots {
    pub robot_state: Option<Vec<u8>>,
    pub safety_state: Option<Vec<u8>>,
    pub heartbeat: Option<Vec<u8>>,
    pub imu_torso: Option<Vec<u8>>,
    pub host_metrics_pi: Option<Vec<u8>>,
    pub host_metrics_jetson: Option<Vec<u8>>,
    pub actuator_limits: Option<Vec<u8>>,
}

pub struct AppState {
    pub bus: Arc<Bus>,
    pub snapshots: Arc<RwLock<Snapshots>>,
    pub ipc: Option<Arc<IpcListener>>,
    pub logs: Option<Arc<LogSvc>>,
    /// Base64 SHA-256 of the DER WebTransport cert (for Consul `serverCertificateHashes`).
    pub tls_cert_sha256_base64: RwLock<Option<String>>,
    envelope_tx: broadcast::Sender<(String, Vec<u8>)>,
    pub rate_limiter: RateLimiter,
    /// Command-eligible joints from the active bringup profile.
    pub command_joints: CommandJointAllowlist,
    /// True after a live apply whose write-behind failed (distinct from NeedsRestart).
    persist_degraded: AtomicBool,
    /// True while a config persist is pending (restart must wait / refuse).
    persist_pending: AtomicBool,
    runtime_generation: std::sync::atomic::AtomicU64,
}

impl AppState {
    /// IPC connection changes retire all prior producer observations. Connectivity
    /// itself provides no replacement safety or motion-admission evidence.
    pub fn runtime_connection_changed(&self, connected: bool) {
        if let Ok(mut snapshots) = self.snapshots.write() {
            *snapshots = Snapshots::default();
        }
        let generation = self.runtime_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let message = armee_proto::RuntimeConnectionState {
            generation,
            connected,
        };
        let envelope = armee_proto::Envelope {
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|now| now.as_millis() as u64)
                .unwrap_or(0),
            source_node: "marengo-gateway".into(),
            message_type: "marengo.v1.RuntimeConnectionState".into(),
            payload: message.encode_to_vec(),
        };
        let _ = self
            .envelope_tx
            .send((TOPIC_RUNTIME_CONNECTION.into(), envelope.encode_to_vec()));
    }

    pub fn new(bus: Arc<Bus>) -> Self {
        let (envelope_tx, _) = broadcast::channel(ENVELOPE_BROADCAST_CAPACITY);
        Self {
            bus,
            snapshots: Arc::new(RwLock::new(Snapshots::default())),
            ipc: None,
            logs: None,
            tls_cert_sha256_base64: RwLock::new(None),
            envelope_tx,
            rate_limiter: RateLimiter::new(),
            command_joints: CommandJointAllowlist::empty(),
            persist_degraded: AtomicBool::new(false),
            persist_pending: AtomicBool::new(false),
            runtime_generation: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn persist_degraded(&self) -> bool {
        self.persist_degraded.load(Ordering::Relaxed)
    }

    pub fn persist_pending(&self) -> bool {
        self.persist_pending.load(Ordering::Relaxed)
    }

    pub fn with_command_joints(mut self, command_joints: CommandJointAllowlist) -> Self {
        self.command_joints = command_joints;
        self
    }

    pub fn with_logs(mut self, logs: LogSvc) -> Self {
        self.logs = Some(Arc::new(logs));
        self
    }

    pub fn set_tls_cert_sha256_base64(&self, value: String) {
        if let Ok(mut guard) = self.tls_cert_sha256_base64.write() {
            *guard = Some(value);
        }
    }

    pub fn tls_cert_sha256_base64(&self) -> Option<String> {
        self.tls_cert_sha256_base64
            .read()
            .ok()
            .and_then(|g| g.clone())
    }

    pub fn with_ipc(mut self, ipc: Arc<IpcListener>) -> Self {
        self.ipc = Some(ipc);
        self
    }

    /// Process a single frame: update the snapshot, persist structured logs, and
    /// fan it out to WebTransport subscribers via `envelope_tx`.
    ///
    /// This must NOT re-publish onto `self.bus`. The bus fanout task
    /// (`spawn_bus_fanout`) is itself a bus subscriber, so publishing back here
    /// would feed every frame straight into an infinite echo loop — each cycle
    /// queuing another unbounded DB insert until the process exhausts memory.
    /// Frames already reach the bus from their real sources (the IPC listener
    /// and the gateway's own `ChappeLogLayer`); this handler is the sink.
    pub fn ingest_runtime_frame(&self, topic: String, payload: Vec<u8>) {
        if topic == TOPIC_LOGS {
            if let (Some(logs), Some(event)) = (&self.logs, decode_log_payload(&payload)) {
                logs.ingest_log_event(&event);
            }
        }
        if topic == TOPIC_AUDIT_ACTION {
            if let Ok(event) = decode_envelope_payload::<ActionEvent>(&payload) {
                match PersistStatus::try_from(event.persist_status) {
                    Ok(PersistStatus::Pending) => {
                        self.persist_pending.store(true, Ordering::Relaxed);
                    }
                    Ok(PersistStatus::Durable) => {
                        self.persist_pending.store(false, Ordering::Relaxed);
                        self.persist_degraded.store(false, Ordering::Relaxed);
                    }
                    Ok(PersistStatus::Failed) => {
                        self.persist_pending.store(false, Ordering::Relaxed);
                        self.persist_degraded.store(true, Ordering::Relaxed);
                    }
                    _ => {}
                }
            }
        }
        self.update_snapshot(&topic, &payload);
        let _ = self.envelope_tx.send((topic.clone(), payload.clone()));
        // Fan RobotState to testing telemetry topic so the Testing page has
        // a dedicated subscription without sharing the main dashboard's topic.
        if topic == TOPIC_STATE {
            let _ = self
                .envelope_tx
                .send((TOPIC_TESTING_TELEMETRY.to_string(), payload));
        }
    }

    fn update_snapshot(&self, topic: &str, payload: &[u8]) {
        let mut guard = match self.snapshots.write() {
            Ok(g) => g,
            Err(_) => return,
        };
        match topic {
            TOPIC_STATE => guard.robot_state = Some(payload.to_vec()),
            TOPIC_SAFETY => guard.safety_state = Some(payload.to_vec()),
            TOPIC_HEARTBEAT => guard.heartbeat = Some(payload.to_vec()),
            TOPIC_IMU_TORSO => guard.imu_torso = Some(payload.to_vec()),
            TOPIC_HOST_METRICS_PI => guard.host_metrics_pi = Some(payload.to_vec()),
            TOPIC_HOST_METRICS_JETSON => guard.host_metrics_jetson = Some(payload.to_vec()),
            TOPIC_ACTUATOR_LIMITS => guard.actuator_limits = Some(payload.to_vec()),
            _ => {}
        }
    }

    pub fn subscribe_envelopes(&self) -> broadcast::Receiver<(String, Vec<u8>)> {
        self.envelope_tx.subscribe()
    }

    pub fn publish_command_envelope(
        &self,
        topic: &str,
        source_node: &str,
        message_type: &str,
        payload: Vec<u8>,
    ) -> Result<(), String> {
        use armee_proto::prost::Message;
        let envelope = armee_proto::Envelope {
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            source_node: source_node.to_string(),
            message_type: message_type.to_string(),
            payload,
        };
        let bytes = envelope.encode_to_vec();
        if let Some(ipc) = &self.ipc {
            ipc.send_command(topic, &bytes).map_err(|e| e.to_string())?;
        }
        self.bus
            .publish_bytes(topic, bytes)
            .map_err(|e| e.to_string())
    }

    pub fn snapshot_robot_state(&self) -> Option<RobotState> {
        let bytes = self.snapshots.read().ok()?.robot_state.clone()?;
        decode_envelope_payload::<RobotState>(&bytes).ok()
    }

    pub fn snapshot_safety(&self) -> Option<SafetyState> {
        let bytes = self.snapshots.read().ok()?.safety_state.clone()?;
        decode_envelope_payload::<SafetyState>(&bytes).ok()
    }

    pub fn snapshot_heartbeat(&self) -> Option<Heartbeat> {
        let bytes = self.snapshots.read().ok()?.heartbeat.clone()?;
        decode_envelope_payload::<Heartbeat>(&bytes).ok()
    }

    pub fn snapshot_imu_torso(&self) -> Option<ImuSample> {
        let bytes = self.snapshots.read().ok()?.imu_torso.clone()?;
        decode_envelope_payload::<ImuSample>(&bytes).ok()
    }

    pub fn snapshot_host_metrics_pi(&self) -> Option<HostMetrics> {
        let bytes = self.snapshots.read().ok()?.host_metrics_pi.clone()?;
        decode_envelope_payload::<HostMetrics>(&bytes).ok()
    }

    pub fn snapshot_host_metrics_jetson(&self) -> Option<HostMetrics> {
        let bytes = self.snapshots.read().ok()?.host_metrics_jetson.clone()?;
        decode_envelope_payload::<HostMetrics>(&bytes).ok()
    }

    pub fn snapshot_actuator_limits(&self) -> Option<ActuatorLimitSnapshot> {
        let bytes = self.snapshots.read().ok()?.actuator_limits.clone()?;
        decode_envelope_payload::<ActuatorLimitSnapshot>(&bytes).ok()
    }
}

fn decode_envelope_payload<M: armee_proto::prost::Message + Default>(
    envelope_bytes: &[u8],
) -> Result<M, armee_proto::prost::DecodeError> {
    let env = armee_proto::Envelope::decode(envelope_bytes)?;
    M::decode(env.payload.as_slice())
}

pub fn topic_allowed(topic: &str) -> bool {
    ALLOWED_TOPICS.contains(&topic)
}

pub fn filter_topics(topics: &[String]) -> Vec<String> {
    let mut allowed: Vec<String> = topics
        .iter()
        .filter(|t| topic_allowed(t))
        .cloned()
        .collect();
    if !allowed.is_empty()
        && !allowed
            .iter()
            .any(|topic| topic == TOPIC_RUNTIME_CONNECTION)
    {
        allowed.push(TOPIC_RUNTIME_CONNECTION.into());
    }
    allowed
}

pub type SharedState = Arc<AppState>;

pub fn spawn_bus_fanout(state: SharedState) {
    for topic in ALLOWED_TOPICS {
        let st = Arc::clone(&state);
        let topic = topic.to_string();
        let mut rx = state.bus.subscribe(&topic);
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(bytes) => st.ingest_runtime_frame(topic.clone(), bytes),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }
}

#[cfg(test)]
mod ipc_snapshot_tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use armee_proto::Envelope;

    #[test]
    fn connection_change_retires_observations_until_a_new_frame_arrives() {
        let state = AppState::new(Arc::new(Bus::default()));
        let frame = Envelope {
            timestamp_ms: 1,
            source_node: "retired-pi".into(),
            message_type: "marengo.v1.RobotState".into(),
            payload: RobotState {
                timestamp_ms: 42,
                joints: vec![],
            }
            .encode_to_vec(),
        }
        .encode_to_vec();
        state.ingest_runtime_frame(TOPIC_STATE.into(), frame.clone());
        assert_eq!(
            state
                .snapshot_robot_state()
                .expect("old observation")
                .timestamp_ms,
            42
        );
        state.runtime_connection_changed(false);
        assert!(state.snapshot_robot_state().is_none());
        assert!(state.snapshot_safety().is_none());
        assert!(state.snapshot_heartbeat().is_none());
        assert!(state.snapshot_host_metrics_pi().is_none());
        state.ingest_runtime_frame(TOPIC_STATE.into(), frame);
        assert!(state.snapshot_robot_state().is_some());
        assert!(
            state.snapshot_safety().is_none(),
            "state does not synthesize safety evidence"
        );
        state.runtime_connection_changed(false);
        assert!(state.snapshot_robot_state().is_none());
    }
}

#[cfg(test)]
mod runtime_transition_stream_tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use armee_proto::{Envelope, RuntimeConnectionState};
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn existing_topic_subscriber_receives_typed_runtime_invalidation() {
        let state = AppState::new(Arc::new(Bus::default()));
        let topics = filter_topics(&[TOPIC_STATE.into()]);
        assert!(topics.iter().any(|topic| topic == TOPIC_RUNTIME_CONNECTION));
        let rx = state.subscribe_envelopes();
        let (mut writer, mut reader) = tokio::io::duplex(4096);
        let pump = tokio::spawn(async move {
            crate::framing::pump_envelope_stream(rx, &topics, &mut writer).await
        });
        state.runtime_connection_changed(false);
        let length = tokio::time::timeout(std::time::Duration::from_secs(5), reader.read_u32_le())
            .await
            .expect("stream deadline")
            .expect("frame length") as usize;
        assert!(length < 4096);
        let mut bytes = vec![0; length];
        reader.read_exact(&mut bytes).await.expect("stream payload");
        let envelope = Envelope::decode(bytes.as_slice()).expect("wire envelope");
        assert_eq!(envelope.message_type, "marengo.v1.RuntimeConnectionState");
        let transition =
            RuntimeConnectionState::decode(envelope.payload.as_slice()).expect("typed transition");
        assert_eq!((transition.generation, transition.connected), (1, false));
        pump.abort();
        assert!(pump
            .await
            .expect_err("owned stream task cancelled")
            .is_cancelled());
    }
}
