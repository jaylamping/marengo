//! Publish `tracing` events as Chappe `LogEvent` on `logs/structured`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::Bus;
use armee_proto::LogEvent;
use serde_json::{Map, Value};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

pub const TOPIC_LOGS: &str = "logs/structured";

const MAX_LOGS_PER_SEC: u64 = 40;
const MAX_FIELDS_JSON_BYTES: usize = 2048;

/// Rate-limited layer forwarding tracing events to Chappe.
pub struct ChappeLogLayer {
    bus: Arc<Bus>,
    source_node: String,
    window_start: AtomicU64,
    window_count: AtomicU64,
}

impl ChappeLogLayer {
    pub fn new(bus: Arc<Bus>, source_node: impl Into<String>) -> Self {
        Self {
            bus,
            source_node: source_node.into(),
            window_start: AtomicU64::new(0),
            window_count: AtomicU64::new(0),
        }
    }

    fn allow_event(&self, level: Level) -> bool {
        let now = Instant::now().elapsed().as_millis() as u64;
        let start = self.window_start.load(Ordering::Relaxed);
        if now.saturating_sub(start) >= 1000 {
            self.window_start.store(now, Ordering::Relaxed);
            self.window_count.store(0, Ordering::Relaxed);
        }
        let count = self.window_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count > MAX_LOGS_PER_SEC {
            return matches!(level, Level::ERROR | Level::WARN);
        }
        true
    }
}

struct FieldsVisitor {
    message: String,
    fields: Map<String, Value>,
}

impl FieldsVisitor {
    fn insert_field(&mut self, name: &str, value: Value) {
        if name != "message" {
            self.fields.insert(name.to_string(), value);
        }
    }
}

impl Visit for FieldsVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}").trim_matches('"').to_string();
        } else {
            self.insert_field(
                field.name(),
                Value::String(format!("{value:?}").trim_matches('"').to_string()),
            );
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.insert_field(field.name(), Value::String(value.to_string()));
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.insert_field(field.name(), Value::Bool(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.insert_field(field.name(), Value::Number(value.into()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.insert_field(field.name(), Value::Number(value.into()));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if let Some(number) = serde_json::Number::from_f64(value) {
            self.insert_field(field.name(), Value::Number(number));
        } else {
            self.insert_field(field.name(), Value::String(value.to_string()));
        }
    }
}

fn serialize_fields_json(fields: &mut Map<String, Value>) -> String {
    fields.remove("_truncated");
    let initial = serde_json::to_string(fields).unwrap_or_default();
    if initial.len() <= MAX_FIELDS_JSON_BYTES {
        return initial;
    }
    while serde_json::to_string(fields)
        .map(|json| json.len())
        .unwrap_or(usize::MAX)
        > MAX_FIELDS_JSON_BYTES
    {
        if fields.is_empty() {
            return r#"{"_truncated":true}"#.to_string();
        }
        // BTreeMap key order — never pop `.next()` once `_truncated` exists; drop from the end.
        let Some(key) = fields.keys().next_back().cloned() else {
            break;
        };
        fields.remove(&key);
    }
    fields.insert("_truncated".to_string(), Value::Bool(true));
    serde_json::to_string(fields).unwrap_or_else(|_| r#"{"_truncated":true}"#.to_string())
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::TRACE => "trace",
        Level::DEBUG => "debug",
        Level::INFO => "info",
        Level::WARN => "warn",
        Level::ERROR => "error",
    }
}

impl<S> Layer<S> for ChappeLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let level = *event.metadata().level();
        if !self.allow_event(level) {
            return;
        }
        let mut visitor = FieldsVisitor {
            message: String::new(),
            fields: Map::new(),
        };
        event.record(&mut visitor);
        if visitor.message.is_empty() {
            visitor.message = event.metadata().name().to_string();
        }
        let fields_json = if visitor.fields.is_empty() {
            String::new()
        } else {
            serialize_fields_json(&mut visitor.fields)
        };
        let log = LogEvent {
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            level: level_name(level).to_string(),
            target: event.metadata().target().to_string(),
            message: visitor.message,
            session_id: std::env::var("MARENGO_LOG_SESSION_ID").unwrap_or_default(),
            fields_json,
        };
        let _ = self
            .bus
            .publish(TOPIC_LOGS, &self.source_node, "marengo.v1.LogEvent", &log);
    }
}

/// Initialize fmt + env filter + optional Chappe log layer (call once from bin main).
pub fn init_subscriber(bus: Option<Arc<Bus>>, source_node: &str) {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::from_default_env();
    if let Some(bus) = bus {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer())
            .with(ChappeLogLayer::new(bus, source_node))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn level_names_match_proto_contract() {
        assert_eq!(level_name(Level::INFO), "info");
        assert_eq!(level_name(Level::ERROR), "error");
    }

    #[test]
    fn serialize_fields_truncates_large_payload() {
        let mut fields = Map::new();
        for i in 0..500 {
            fields.insert(format!("field_{i}"), Value::String("x".repeat(20)));
        }
        let json = serialize_fields_json(&mut fields);
        assert!(json.len() <= MAX_FIELDS_JSON_BYTES);
        assert!(fields.contains_key("_truncated"));
    }

    #[test]
    fn build_fields_json_from_map() {
        let mut fields = Map::new();
        fields.insert("joint".into(), Value::String("shoulder_pitch".into()));
        fields.insert("device_id".into(), Value::Number(7.into()));
        let json = serialize_fields_json(&mut fields);
        assert!(json.contains("shoulder_pitch"));
        assert!(json.contains("device_id"));
    }
}
