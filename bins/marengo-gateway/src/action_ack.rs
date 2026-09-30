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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::time::Duration;

    use armee_proto::{ActionEvent, PersistStatus as ProtoPersistStatus};
    use tokio::time::timeout;

    use super::*;
    use crate::state::TOPIC_AUDIT_ACTION;

    fn sample_action(
        session_id: &str,
        action: &str,
        accepted: bool,
        persist: ProtoPersistStatus,
    ) -> ActionEvent {
        ActionEvent {
            timestamp_ms: 1,
            session_id: session_id.to_string(),
            operator_id: "op".to_string(),
            joint: "right_elbow_pitch".to_string(),
            action: action.to_string(),
            revision: 1,
            accepted,
            reject_reason: if accepted {
                String::new()
            } else {
                "rejected".to_string()
            },
            persist_status: persist as i32,
            config_revision: "rev-1".to_string(),
        }
    }

    fn is_live_limit_patch_ack(session_id: &str, event: &ActionEvent) -> bool {
        event.session_id == session_id && event.action == LIVE_LIMIT_PATCH_ACTION
    }

    #[test]
    fn live_ack_matcher_ignores_write_behind_persist_action() {
        let session = "sess-1";
        assert!(is_live_limit_patch_ack(
            session,
            &sample_action(session, "limit_patch", true, ProtoPersistStatus::Pending)
        ));
        assert!(!is_live_limit_patch_ack(
            session,
            &sample_action(
                session,
                "limit_patch_persist",
                false,
                ProtoPersistStatus::Failed
            )
        ));
        assert!(!is_live_limit_patch_ack(
            "other",
            &sample_action(session, "limit_patch", true, ProtoPersistStatus::Pending)
        ));
    }

    #[tokio::test]
    async fn recv_live_ack_skips_persist_failed_until_pending() {
        let bus = std::sync::Arc::new(chappe::Bus::new(64));
        let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
        let mut rx = state.subscribe_envelopes();
        let session = "sess-live";
        state.ingest_runtime_frame(
            TOPIC_AUDIT_ACTION.to_string(),
            encode_action_event_envelope(&sample_action(
                session,
                "limit_patch_persist",
                false,
                ProtoPersistStatus::Failed,
            )),
        );
        state.ingest_runtime_frame(
            TOPIC_AUDIT_ACTION.to_string(),
            encode_action_event_envelope(&sample_action(
                session,
                "limit_patch",
                true,
                ProtoPersistStatus::Pending,
            )),
        );
        let event = timeout(
            Duration::from_secs(1),
            recv_action_ack(
                &mut rx,
                session,
                "right_elbow_pitch",
                "op",
                LIVE_LIMIT_PATCH_ACTION,
            ),
        )
        .await
        .expect("ack");
        assert!(event.accepted);
        assert_eq!(event.action, "limit_patch");
        assert_eq!(event.persist_status, ProtoPersistStatus::Pending as i32);
    }

    #[tokio::test]
    async fn recv_live_ack_surfaces_live_reject() {
        let bus = std::sync::Arc::new(chappe::Bus::new(64));
        let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
        let mut rx = state.subscribe_envelopes();
        let session = "sess-reject";
        state.ingest_runtime_frame(
            TOPIC_AUDIT_ACTION.to_string(),
            encode_action_event_envelope(&sample_action(
                session,
                "limit_patch",
                false,
                ProtoPersistStatus::NotApplicable,
            )),
        );
        let event = timeout(
            Duration::from_secs(1),
            recv_action_ack(
                &mut rx,
                session,
                "right_elbow_pitch",
                "op",
                LIVE_LIMIT_PATCH_ACTION,
            ),
        )
        .await
        .expect("ack");
        assert!(!event.accepted);
        assert_eq!(event.reject_reason, "rejected");
    }

    #[tokio::test]
    async fn recv_live_ack_times_out_without_matching_session() {
        let bus = std::sync::Arc::new(chappe::Bus::new(64));
        let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
        let mut rx = state.subscribe_envelopes();
        state.ingest_runtime_frame(
            TOPIC_AUDIT_ACTION.to_string(),
            encode_action_event_envelope(&sample_action(
                "other-session",
                "limit_patch",
                true,
                ProtoPersistStatus::Pending,
            )),
        );
        let timed_out = timeout(
            Duration::from_millis(50),
            recv_action_ack(
                &mut rx,
                "wanted-session",
                "right_elbow_pitch",
                "op",
                LIVE_LIMIT_PATCH_ACTION,
            ),
        )
        .await;
        assert!(
            timed_out.is_err(),
            "mismatched session must not satisfy wait"
        );
    }
}
