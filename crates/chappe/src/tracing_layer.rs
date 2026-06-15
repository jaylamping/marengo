//! Publish `tracing` events as Chappe `LogEvent` on `logs/structured`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::Bus;
use armee_proto::LogEvent;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

pub const TOPIC_LOGS: &str = "logs/structured";

const MAX_LOGS_PER_SEC: u64 = 40;

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

struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}").trim_matches('"').to_string();
        }
    }
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
        let mut visitor = MessageVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);
        if visitor.message.is_empty() {
            visitor.message = event.metadata().name().to_string();
        }
        let log = LogEvent {
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            level: level_name(level).to_string(),
            target: event.metadata().target().to_string(),
            message: visitor.message,
            session_id: std::env::var("MARENGO_LOG_SESSION_ID").unwrap_or_default(),
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
}
