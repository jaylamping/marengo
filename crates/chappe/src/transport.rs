//! Transport abstraction for Chappe publishers/subscribers.

use std::sync::Arc;

use armee_proto::prost::Message;
use thiserror::Error;

use crate::{Bus, BusError};

/// Publish and subscribe to Chappe topics (in-process or bridged).
pub trait Transport: Send + Sync {
    fn publish_bytes(&self, topic: &str, payload: Vec<u8>) -> Result<(), BusError>;

    fn publish<M: Message>(
        &self,
        topic: &str,
        source_node: &str,
        message_type: &str,
        message: &M,
    ) -> Result<(), BusError> {
        let payload = message.encode_to_vec();
        let envelope = armee_proto::Envelope {
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

    fn subscribe(&self, topic: &str) -> tokio::sync::broadcast::Receiver<Vec<u8>>;
}

impl Transport for Bus {
    fn publish_bytes(&self, topic: &str, payload: Vec<u8>) -> Result<(), BusError> {
        Bus::publish_bytes(self, topic, payload)
    }

    fn subscribe(&self, topic: &str) -> tokio::sync::broadcast::Receiver<Vec<u8>> {
        Bus::subscribe(self, topic)
    }
}

/// [`Bus`] plus optional IPC fanout for multi-process gateways.
#[derive(Clone)]
pub struct SharedBus {
    bus: Bus,
    ipc: Option<Arc<crate::ipc::IpcFanout>>,
}

impl SharedBus {
    pub fn new(capacity: usize) -> Self {
        Self {
            bus: Bus::new(capacity),
            ipc: None,
        }
    }

    pub fn with_ipc_fanout(mut self, ipc: Arc<crate::ipc::IpcFanout>) -> Self {
        self.ipc = Some(ipc);
        self
    }

    pub fn inner(&self) -> &Bus {
        &self.bus
    }
}

impl Transport for SharedBus {
    fn publish_bytes(&self, topic: &str, payload: Vec<u8>) -> Result<(), BusError> {
        if let Some(ipc) = &self.ipc {
            ipc.forward_runtime_to_gateway(topic, &payload);
        }
        Bus::publish_bytes(&self.bus, topic, payload)
    }

    fn subscribe(&self, topic: &str) -> tokio::sync::broadcast::Receiver<Vec<u8>> {
        Bus::subscribe(&self.bus, topic)
    }
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("bus: {0}")]
    Bus(#[from] BusError),
    #[error("ipc: {0}")]
    Ipc(String),
}
