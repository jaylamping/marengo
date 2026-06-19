//! Anonymous session UUID minting for actuator harness audit anchors.

use std::time::{SystemTime, UNIX_EPOCH};

use armee_proto::{SessionStartRequest, SessionStartResponse};
use uuid::Uuid;

/// Mint a new v4 session UUID for one operator connection.
pub fn mint_session_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn build_session_start_response(request: &SessionStartRequest) -> SessionStartResponse {
    let _ = request;
    SessionStartResponse {
        session_id: mint_session_id(),
        started_ms: now_ms(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn mint_session_id_is_uuid_v4() {
        let id = mint_session_id();
        let parsed = Uuid::parse_str(&id).expect("valid uuid");
        assert_eq!(parsed.get_version(), Some(uuid::Version::Random));
    }

    #[test]
    fn mint_session_id_unique_per_call() {
        let a = mint_session_id();
        let b = mint_session_id();
        assert_ne!(a, b);
    }
}
