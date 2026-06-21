use std::sync::{Arc, RwLock};

use armee_proto::prost::Message;
use armee_proto::{Heartbeat, HostMetrics, ImuSample, RobotState, SafetyState};
use chappe::ipc::IpcListener;
use chappe::Bus;
use tokio::sync::broadcast;

use crate::logs::{decode_log_payload, LogServices as LogSvc};

pub const TOPIC_STATE: &str = "robot/state";
pub const TOPIC_SAFETY: &str = "robot/safety";
pub const TOPIC_HEARTBEAT: &str = "robot/heartbeat";
pub const TOPIC_IMU_TORSO: &str = "sensors/imu/torso";
pub const TOPIC_LOGS: &str = "logs/structured";
pub const TOPIC_HOST_METRICS_PI: &str = "host/metrics/pi";
pub const TOPIC_HOST_METRICS_JETSON: &str = "host/metrics/jetson";
pub const TOPIC_TESTING_MIT_COMMAND_BATCH: &str = "robot/testing/mit_command_batch";
pub const TOPIC_TESTING_TELEMETRY: &str = "robot/testing/telemetry";

pub const ALLOWED_TOPICS: &[&str] = &[
    TOPIC_STATE,
    TOPIC_SAFETY,
    TOPIC_HEARTBEAT,
    TOPIC_IMU_TORSO,
    TOPIC_LOGS,
    TOPIC_HOST_METRICS_PI,
    TOPIC_HOST_METRICS_JETSON,
    TOPIC_TESTING_MIT_COMMAND_BATCH,
    TOPIC_TESTING_TELEMETRY,
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
}

pub struct AppState {
    pub bus: Arc<Bus>,
    pub snapshots: Arc<RwLock<Snapshots>>,
    pub ipc: Option<Arc<IpcListener>>,
    pub logs: Option<Arc<LogSvc>>,
    /// Base64 SHA-256 of the DER WebTransport cert (for Consul `serverCertificateHashes`).
    pub tls_cert_sha256_base64: RwLock<Option<String>>,
    envelope_tx: broadcast::Sender<(String, Vec<u8>)>,
}

impl AppState {
    pub fn new(bus: Arc<Bus>) -> Self {
        let (envelope_tx, _) = broadcast::channel(ENVELOPE_BROADCAST_CAPACITY);
        Self {
            bus,
            snapshots: Arc::new(RwLock::new(Snapshots::default())),
            ipc: None,
            logs: None,
            tls_cert_sha256_base64: RwLock::new(None),
            envelope_tx,
        }
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
        self.update_snapshot(&topic, &payload);
        let _ = self.envelope_tx.send((topic.clone(), payload.clone()));
        // Fan RobotState to testing telemetry topic so the Testing page has
        // a dedicated subscription without sharing the main dashboard's topic.
        if topic == TOPIC_STATE {
            let _ = self.envelope_tx.send((TOPIC_TESTING_TELEMETRY.to_string(), payload));
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
    topics
        .iter()
        .filter(|t| topic_allowed(t))
        .cloned()
        .collect()
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
