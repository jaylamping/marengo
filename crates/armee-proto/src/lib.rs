//! Generated Protocol Buffer types from [`proto/`](../../proto/).
//!
//! Do not hand-edit this crate — change `.proto` files and rebuild.

include!(concat!(env!("OUT_DIR"), "/marengo.v1.rs"));

pub use prost;

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::prost::Message;
    use super::{
        EnableRequest, Envelope, Fault, FaultSeverity, Heartbeat, JointState, OperationalMode,
        RobotState, SafetyState,
    };

    #[test]
    fn heartbeat_roundtrip() {
        let msg = Heartbeat {
            timestamp_ms: 1_700_000_000_000,
            node_id: "marengo-pi".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = Heartbeat::decode(bytes.as_slice()).expect("decode");
        assert_eq!(decoded.timestamp_ms, msg.timestamp_ms);
        assert_eq!(decoded.node_id, msg.node_id);
    }

    #[test]
    fn robot_state_roundtrip() {
        let msg = RobotState {
            timestamp_ms: 42,
            joints: vec![JointState {
                name: "joint1".to_string(),
                position: 0.1,
                velocity: 0.0,
                effort: 0.0,
            }],
        };
        let bytes = msg.encode_to_vec();
        let decoded = RobotState::decode(bytes.as_slice()).expect("decode");
        assert_eq!(decoded.joints.len(), 1);
        assert!((decoded.joints[0].position - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn safety_state_roundtrip() {
        let msg = SafetyState {
            timestamp_ms: 1,
            mode: OperationalMode::Ready as i32,
            hardware_estop_asserted: false,
            software_estop_latched: false,
            active_faults: vec![Fault {
                code: "LIMIT".to_string(),
                message: "test".to_string(),
                severity: FaultSeverity::Warning as i32,
                joint: "joint1".to_string(),
            }],
        };
        let bytes = msg.encode_to_vec();
        let decoded = SafetyState::decode(bytes.as_slice()).expect("decode");
        assert_eq!(decoded.mode, OperationalMode::Ready as i32);
        assert_eq!(decoded.active_faults.len(), 1);
    }

    #[test]
    fn enable_request_roundtrip() {
        let msg = EnableRequest {
            timestamp_ms: 2,
            operator_id: "bench".to_string(),
            enable: true,
        };
        let bytes = msg.encode_to_vec();
        let decoded = EnableRequest::decode(bytes.as_slice()).expect("decode");
        assert!(decoded.enable);
    }

    #[test]
    fn envelope_roundtrip() {
        let inner = Heartbeat {
            timestamp_ms: 1,
            node_id: "probe".to_string(),
        };
        let payload = inner.encode_to_vec();
        let msg = Envelope {
            timestamp_ms: 1,
            source_node: "probe".to_string(),
            message_type: "marengo.v1.Heartbeat".to_string(),
            payload,
        };
        let bytes = msg.encode_to_vec();
        let decoded = Envelope::decode(bytes.as_slice()).expect("decode");
        let hb = Heartbeat::decode(decoded.payload.as_slice()).expect("inner");
        assert_eq!(hb.node_id, "probe");
    }
}
