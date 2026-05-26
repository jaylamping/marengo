use std::sync::{Arc, RwLock};

use armee_proto::prost::Message;
use armee_proto::{Heartbeat, RobotState, SafetyState};
use chappe::ipc::IpcListener;
use chappe::Bus;
use tokio::sync::broadcast;

pub const TOPIC_STATE: &str = "robot/state";
pub const TOPIC_SAFETY: &str = "robot/safety";
pub const TOPIC_HEARTBEAT: &str = "robot/heartbeat";
pub const TOPIC_LOGS: &str = "logs/structured";

pub const ALLOWED_TOPICS: &[&str] = &[TOPIC_STATE, TOPIC_SAFETY, TOPIC_HEARTBEAT, TOPIC_LOGS];

#[derive(Default, Clone)]
pub struct Snapshots {
    pub robot_state: Option<Vec<u8>>,
    pub safety_state: Option<Vec<u8>>,
    pub heartbeat: Option<Vec<u8>>,
}

pub struct AppState {
    pub bus: Arc<Bus>,
    pub snapshots: Arc<RwLock<Snapshots>>,
    pub ipc: Option<Arc<IpcListener>>,
    envelope_tx: broadcast::Sender<(String, Vec<u8>)>,
}

impl AppState {
    pub fn new(bus: Arc<Bus>) -> Self {
        let (envelope_tx, _) = broadcast::channel(512);
        Self {
            bus,
            snapshots: Arc::new(RwLock::new(Snapshots::default())),
            ipc: None,
            envelope_tx,
        }
    }

    pub fn with_ipc(mut self, ipc: Arc<IpcListener>) -> Self {
        self.ipc = Some(ipc);
        self
    }

    pub fn ingest_runtime_frame(&self, topic: String, payload: Vec<u8>) {
        self.update_snapshot(&topic, &payload);
        let _ = self.bus.publish_bytes(&topic, payload.clone());
        let _ = self.envelope_tx.send((topic, payload));
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
