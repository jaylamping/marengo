//! Publish `tracing` events as Chappe `LogEvent` on `logs/structured`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
const QUOTA_COUNT_BITS: u32 = 8;
const QUOTA_COUNT_MASK: u64 = (1 << QUOTA_COUNT_BITS) - 1;

/// Rate-limited layer forwarding tracing events to Chappe.
pub struct ChappeLogLayer {
    bus: Arc<Bus>,
    source_node: String,
    // Store the elapsed-second bucket and its count together so concurrent events
    // cannot reset one another's quota or mix counts from different windows.
    quota: AtomicU64,
    elapsed: Arc<dyn Fn() -> Duration + Send + Sync>,
}

impl ChappeLogLayer {
    pub fn new(bus: Arc<Bus>, source_node: impl Into<String>) -> Self {
        let origin = Instant::now();
        Self::with_clock(bus, source_node, move || origin.elapsed())
    }

    fn with_clock(
        bus: Arc<Bus>,
        source_node: impl Into<String>,
        elapsed: impl Fn() -> Duration + Send + Sync + 'static,
    ) -> Self {
        Self {
            bus,
            source_node: source_node.into(),
            quota: AtomicU64::new(0),
            elapsed: Arc::new(elapsed),
        }
    }

    fn allow_event(&self, level: Level) -> bool {
        if matches!(level, Level::ERROR | Level::WARN) {
            return true;
        }
        let bucket = (self.elapsed)().as_secs().min(u64::MAX >> QUOTA_COUNT_BITS);
        let mut current = self.quota.load(Ordering::Relaxed);
        loop {
            let next = if current >> QUOTA_COUNT_BITS < bucket {
                (bucket << QUOTA_COUNT_BITS) | 1
            } else if current & QUOTA_COUNT_MASK >= MAX_LOGS_PER_SEC {
                return false;
            } else {
                current + 1
            };
            match self.quota.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(updated) => current = updated,
            }
        }
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
    use armee_proto::{prost::Message, Envelope};
    use tracing_subscriber::prelude::*;

    fn published_logs(rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>) -> Vec<LogEvent> {
        let mut logs = Vec::new();
        while let Ok(bytes) = rx.try_recv() {
            let envelope = Envelope::decode(bytes.as_slice()).expect("envelope");
            logs.push(LogEvent::decode(envelope.payload.as_slice()).expect("log event"));
        }
        logs
    }

    #[test]
    fn tracing_log_quota_refills_and_preserves_urgent_events() {
        let bus = Arc::new(Bus::new(256));
        let mut rx = bus.subscribe(TOPIC_LOGS);
        let millis = Arc::new(AtomicU64::new(0));
        let clock = Arc::clone(&millis);
        let layer = ChappeLogLayer::with_clock(bus, "test", move || {
            Duration::from_millis(clock.load(Ordering::Relaxed))
        });
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            for sequence in 0..41 {
                tracing::info!(
                    sequence,
                    joint = "shoulder_pitch",
                    device_id = 7,
                    "before refill"
                );
            }
            tracing::warn!("urgent warning");
            tracing::error!("urgent error");
            let first_window = published_logs(&mut rx);
            assert_eq!(first_window.len(), 42);
            assert_eq!(first_window[39].level, "info");
            assert_eq!(first_window[39].message, "before refill");
            let fields: Value =
                serde_json::from_str(&first_window[39].fields_json).expect("structured fields");
            assert_eq!(fields["joint"], "shoulder_pitch");
            assert_eq!(fields["device_id"], 7);
            assert_eq!(first_window[40].level, "warn");
            assert_eq!(first_window[41].level, "error");

            millis.store(999, Ordering::Relaxed);
            tracing::info!("still throttled");
            assert!(published_logs(&mut rx).is_empty());

            millis.store(1000, Ordering::Relaxed);
            for sequence in 0..41 {
                tracing::info!(sequence, "after refill");
            }
            let second_window = published_logs(&mut rx);
            assert_eq!(
                second_window.len(),
                40,
                "normal logging must recover each second"
            );
            assert!(second_window
                .iter()
                .all(|event| event.message == "after refill"));
            assert_eq!(
                serde_json::from_str::<Value>(&second_window[39].fields_json)
                    .expect("structured fields")["sequence"],
                39
            );
        });
    }

    #[test]
    fn concurrent_tracing_events_share_one_quota() {
        let bus = Arc::new(Bus::new(256));
        let mut rx = bus.subscribe(TOPIC_LOGS);
        let layer = ChappeLogLayer::with_clock(bus, "test", || Duration::ZERO);
        let dispatch = tracing::Dispatch::new(tracing_subscriber::registry().with(layer));
        let ready = std::sync::Barrier::new(4);
        std::thread::scope(|scope| {
            for producer in 0..4 {
                let dispatch = &dispatch;
                let ready = &ready;
                scope.spawn(move || {
                    tracing::dispatcher::with_default(dispatch, || {
                        ready.wait();
                        for sequence in 0..32 {
                            tracing::info!(producer, sequence, "concurrent event");
                        }
                    });
                });
            }
        });
        let logs = published_logs(&mut rx);
        assert_eq!(logs.len(), 40);
        let identities: std::collections::HashSet<_> = logs
            .iter()
            .map(|log| {
                let fields: Value = serde_json::from_str(&log.fields_json).expect("fields");
                (fields["producer"].as_i64(), fields["sequence"].as_i64())
            })
            .collect();
        assert_eq!(
            identities.len(),
            40,
            "each admitted event must publish once"
        );
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
}
