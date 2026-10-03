//! Single source of truth for Chappe topic strings (L-chappe-09).
//!
//! The topic contract used to live in three places — `crate::ipc`
//! (commands), `crate::ipc_outbox` (forwarded telemetry), the gateway's
//! `state.rs`, and `marengo-pi`'s `main.rs` — so adding a topic in one file
//! silently stopped forwarding. Every producer, forwarder and subscriber
//! must use these constants; tests keep independent string literals so a
//! changed value fails loudly.

/// Latest-value telemetry: one coalescing slot each in the IPC outbox.
pub const TOPIC_SAFETY: &str = "robot/safety";
pub const TOPIC_HEARTBEAT: &str = "robot/heartbeat";
pub const TOPIC_STATE: &str = "robot/state";
pub const TOPIC_IMU_TORSO: &str = "sensors/imu/torso";
pub const TOPIC_HOST_METRICS_PI: &str = "host/metrics/pi";
pub const TOPIC_ACTUATOR_LIMITS: &str = "robot/actuator/limits";

/// Latest-value telemetry slots, in outbox class order.
pub const LATEST_TELEMETRY_TOPICS: [&str; 6] = [
    TOPIC_SAFETY,
    TOPIC_HEARTBEAT,
    TOPIC_STATE,
    TOPIC_IMU_TORSO,
    TOPIC_HOST_METRICS_PI,
    TOPIC_ACTUATOR_LIMITS,
];

/// Event telemetry: bounded FIFO classes in the IPC outbox.
pub const TOPIC_LOGS: &str = "logs/structured";
pub const TOPIC_AUDIT_ACTION: &str = "robot/audit/action";
pub const TOPIC_AUDIT_TUNING: &str = "robot/audit/tuning";

/// Event telemetry topics, in outbox admission order.
pub const EVENT_TELEMETRY_TOPICS: [&str; 3] = [TOPIC_LOGS, TOPIC_AUDIT_ACTION, TOPIC_AUDIT_TUNING];

/// Gateway → runtime commands (allowlisted, freshness-gated).
pub const TOPIC_ENABLE: &str = "robot/enable";
pub const TOPIC_SET_ZERO: &str = "robot/set_zero";
pub const TOPIC_ACTIVE_REPORTING_LEASE: &str = "robot/active_reporting_lease";
pub const TOPIC_MOTOR_STATUS_POLL: &str = "robot/motor_status_poll";
pub const TOPIC_TESTING_MIT_BATCH: &str = "robot/testing/mit_command_batch";
pub const TOPIC_ACTUATOR_COMMAND: &str = "robot/actuator/command";

/// Gateway → runtime command allowlist.
pub const COMMAND_TOPICS: [&str; 6] = [
    TOPIC_ENABLE,
    TOPIC_SET_ZERO,
    TOPIC_ACTIVE_REPORTING_LEASE,
    TOPIC_MOTOR_STATUS_POLL,
    TOPIC_TESTING_MIT_BATCH,
    TOPIC_ACTUATOR_COMMAND,
];
