//! Wait for Pi audit ActionEvent ACKs on the Chappe fan-in broadcast.

use armee_proto::prost::Message;
use armee_proto::{ActionEvent, Envelope, PersistStatus as ProtoPersistStatus};

use crate::state::TOPIC_AUDIT_ACTION;

/// Live apply ACK only. Write-behind uses `limit_patch_persist` so Failed/Durable
/// cannot be mistaken for a live reject when the Pending event is lagged away.
pub const LIVE_LIMIT_PATCH_ACTION: &str = "limit_patch";
pub const LIMIT_PATCH_PERSIST_ACTION: &str = "limit_patch_persist";

pub async fn recv_action_ack(
    rx: &mut tokio::sync::broadcast::Receiver<(String, Vec<u8>)>,
    session_id: &str,
    joint: &str,
    operator_id: &str,
    action: &str,
) -> ActionEvent {
    loop {
        match rx.recv().await {
            Ok((topic, bytes)) if topic == TOPIC_AUDIT_ACTION => {
                if let Ok(event) = decode_action_event(&bytes) {
                    if event.session_id == session_id && event.action == action {
                        return event;
                    }
                }
            }
            Ok(_) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                return ActionEvent {
                    timestamp_ms: 0,
                    session_id: session_id.to_string(),
                    operator_id: operator_id.to_string(),
                    joint: joint.to_string(),
                    action: action.to_string(),
                    revision: 0,
                    accepted: false,
                    reject_reason: "audit channel closed".to_string(),
                    persist_status: ProtoPersistStatus::Failed as i32,
                    config_revision: String::new(),
                };
            }
        }
    }
}

pub fn decode_action_event(envelope_bytes: &[u8]) -> Result<ActionEvent, ()> {
    let env = Envelope::decode(envelope_bytes).map_err(|_| ())?;
    ActionEvent::decode(env.payload.as_slice()).map_err(|_| ())
}

#[cfg(test)]
pub fn encode_action_event_envelope(event: &ActionEvent) -> Vec<u8> {
    Envelope {
        timestamp_ms: event.timestamp_ms,
        source_node: "test".to_string(),
        message_type: "marengo.v1.ActionEvent".to_string(),
        payload: event.encode_to_vec(),
    }
    .encode_to_vec()
}
