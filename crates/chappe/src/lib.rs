//! # Chappe — inter-process message bus
//!
//! Pub/sub between Pi (`marengo-pi`), Jetson (`marengo-jetson`), Consul UI, and tools.
//! **Transport only** — no motor control, no safety filtering.
//!
//! ## Responsibilities
//!
//! - Topic-based broadcast of protobuf [`Envelope`](armee_proto::Envelope) bytes (ADR 0001).
//! - [`Bus::publish`] / [`Bus::subscribe`] for typed messages (e.g. [`RobotState`](armee_proto::RobotState)).
//!
//! ## Does not
//!
//! - Command motors (Berthier → Davout → robstride).
//! - Parse URDF or load `config/*.yaml` (marengo-config).
//!
//! Typical topics: `robot/state`, telemetry, future RPC. Producers must not put raw CAN on Chappe.

pub mod ipc;
pub mod tracing_layer;
pub mod transport;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use armee_proto::prost::Message;
use armee_proto::Envelope;
use thiserror::Error;
use tokio::sync::broadcast;
pub use transport::{SharedBus, Transport};

const DEFAULT_CAPACITY: usize = 256;

#[derive(Debug, Error)]
pub enum BusError {
    #[error("publish failed: {0}")]
    Publish(String),
    #[error("decode envelope: {0}")]
    Decode(String),
}

/// Topic name → broadcast channel of raw envelope bytes.
#[derive(Clone)]
pub struct Bus {
    inner: Arc<BusInner>,
}

struct BusInner {
    channels: std::sync::RwLock<std::collections::HashMap<String, broadcast::Sender<Vec<u8>>>>,
    capacity: usize,
    last_publish_ms: AtomicU64,
    #[cfg(unix)]
    ipc: std::sync::RwLock<Option<std::sync::Arc<ipc::IpcFanout>>>,
}

impl Default for Bus {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl Bus {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(BusInner {
                channels: std::sync::RwLock::new(std::collections::HashMap::new()),
                capacity,
                last_publish_ms: AtomicU64::new(0),
                #[cfg(unix)]
                ipc: std::sync::RwLock::new(None),
            }),
        }
    }

    /// Forward publishes to a Unix socket for `marengo-gateway` (see ADR 0008).
    #[cfg(unix)]
    pub fn set_ipc_fanout(&self, fanout: std::sync::Arc<ipc::IpcFanout>) {
        if let Ok(mut guard) = self.inner.ipc.write() {
            *guard = Some(fanout);
        }
    }

    fn sender(&self, topic: &str) -> broadcast::Sender<Vec<u8>> {
        let mut guard = self
            .inner
            .channels
            .write()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(tx) = guard.get(topic) {
            return tx.clone();
        }
        let (tx, _) = broadcast::channel(self.inner.capacity);
        guard.insert(topic.to_string(), tx.clone());
        tx
    }

    /// Publish encoded envelope bytes to `topic`.
    ///
    /// Succeeds when there are no subscribers (bench / headless Pi without Consul).
    pub fn publish_bytes(&self, topic: &str, payload: Vec<u8>) -> Result<(), BusError> {
        self.inner.last_publish_ms.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            Ordering::Relaxed,
        );
        #[cfg(unix)]
        if let Ok(guard) = self.inner.ipc.read() {
            if let Some(ipc) = guard.as_ref() {
                ipc.forward_runtime_to_gateway(topic, &payload);
            }
        }
        let tx = self.sender(topic);
        if tx.receiver_count() == 0 {
            return Ok(());
        }
        tx.send(payload)
            .map_err(|e| BusError::Publish(e.to_string()))?;
        Ok(())
    }

    /// Publish a protobuf message wrapped in [`Envelope`].
    pub fn publish<M: Message>(
        &self,
        topic: &str,
        source_node: &str,
        message_type: &str,
        message: &M,
    ) -> Result<(), BusError> {
        let payload = message.encode_to_vec();
        let envelope = Envelope {
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            source_node: source_node.to_string(),
            message_type: message_type.to_string(),
            payload,
        };
        self.publish_bytes(topic, envelope.encode_to_vec())
    }

    /// Subscribe to a topic (receive encoded `Envelope` bytes).
    pub fn subscribe(&self, topic: &str) -> broadcast::Receiver<Vec<u8>> {
        self.sender(topic).subscribe()
    }

    /// Decode the next envelope from a subscription.
    pub async fn recv_envelope(
        rx: &mut broadcast::Receiver<Vec<u8>>,
    ) -> Result<Envelope, BusError> {
        let bytes = rx
            .recv()
            .await
            .map_err(|e| BusError::Decode(e.to_string()))?;
        Envelope::decode(bytes.as_slice()).map_err(|e| BusError::Decode(e.to_string()))
    }

    /// Milliseconds since UNIX epoch of the last successful publish.
    pub fn last_publish_ms(&self) -> u64 {
        self.inner.last_publish_ms.load(Ordering::Relaxed)
    }

    /// Whether IPC fanout is configured (Unix only).
    pub fn ipc_configured(&self) -> bool {
        #[cfg(unix)]
        {
            self.inner.ipc.read().map(|g| g.is_some()).unwrap_or(false)
        }
        #[cfg(not(unix))]
        {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use armee_proto::{prost::Message, Heartbeat};

    use super::*;

    #[tokio::test]
    async fn robot_state_over_chappe_matches_wire_contract() {
        use armee_proto::{JointState, RobotState};

        let bus = Bus::default();
        let mut rx = bus.subscribe("state");
        bus.publish(
            "state",
            "sim",
            "marengo.v1.RobotState",
            &RobotState {
                timestamp_ms: 9,
                joints: vec![JointState {
                    name: "joint1".to_string(),
                    position: 0.0,
                    velocity: 0.0,
                    effort: 0.0,
                    temperature_c: 0.0,
                    fault: 0,
                    homing_state: 0,
                    drive_active: false,
                    out_of_limits: false,
                }],
            },
        )
        .expect("publish");
        let env = Bus::recv_envelope(&mut rx).await.expect("recv");
        let state = RobotState::decode(env.payload.as_slice()).expect("robot state");
        assert_eq!(state.joints.len(), 1);
    }

    #[test]
    fn publish_without_subscribers_succeeds() {
        let bus = Bus::default();
        bus.publish_bytes("robot/state", vec![1, 2, 3])
            .expect("noop publish");
    }

    #[tokio::test]
    async fn publish_subscribe_envelope() {
        let bus = Bus::default();
        let mut rx = bus.subscribe("telemetry");
        bus.publish(
            "telemetry",
            "test",
            "marengo.v1.Heartbeat",
            &Heartbeat {
                timestamp_ms: 1,
                node_id: "probe".to_string(),
            },
        )
        .expect("publish");
        let env = Bus::recv_envelope(&mut rx).await.expect("recv");
        assert_eq!(env.message_type, "marengo.v1.Heartbeat");
        let hb = Heartbeat::decode(env.payload.as_slice()).expect("inner");
        assert_eq!(hb.node_id, "probe");
    }
}
