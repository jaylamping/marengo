//! 1 Hz HostMetrics publisher for Consul host cards.

use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use armee_proto::HostNodeRole;
use chappe::Bus;
use marengo_host_metrics::{
    host_metrics_topic, sample, ChappeHealthInput, IpcQueueHealthInput, SampleState,
};
use tracing::warn;

const SEMVER: &str = env!("CARGO_PKG_VERSION");

pub fn spawn_host_metrics_publisher(chappe: Arc<Bus>, shutdown: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut prev = SampleState::default();
        while !shutdown.load(Ordering::Relaxed) {
            let started = Instant::now();
            let chappe_health = chappe_health_input(chappe.as_ref());
            let metrics = sample(HostNodeRole::Pi, SEMVER, &mut prev, chappe_health);
            if let Err(e) = chappe.publish(
                host_metrics_topic(HostNodeRole::Pi),
                "marengo-pi",
                "marengo.v1.HostMetrics",
                &metrics,
            ) {
                warn!(error = %e, "failed to publish HostMetrics");
            }
            let elapsed = started.elapsed();
            if elapsed < Duration::from_secs(1) {
                thread::sleep(Duration::from_secs(1) - elapsed);
            }
        }
    });
}

fn chappe_health_input(chappe: &Bus) -> ChappeHealthInput {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let last = chappe.last_publish_ms();
    let ipc = chappe.ipc_queue_stats();
    let gateway_probe = probe_gateway_health();
    ChappeHealthInput {
        ipc_connected: ipc.as_ref().is_some_and(|stats| stats.connected),
        gateway_reachable: gateway_probe.is_some(),
        last_publish_age_ms: now_ms.saturating_sub(last),
        gateway_probe_latency_ms: gateway_probe.map(|d| d.as_secs_f64() * 1000.0),
        ipc_queue: ipc.map(|stats| IpcQueueHealthInput {
            queued_items: stats.queued_items as u64,
            queued_payload_bytes: stats.queued_bytes as u64,
            item_capacity: chappe::ipc::QUEUE_ITEM_CAPACITY as u64,
            payload_byte_capacity: chappe::ipc::QUEUE_BYTE_CAPACITY as u64,
            oldest_age_ms: stats.oldest_age_ms,
            accepted_total: stats.accepted,
            coalesced_total: stats.coalesced,
            dropped_total: stats.dropped,
            expired_total: stats.expired,
            rejected_total: stats.rejected,
            admitted_disconnected_total: stats.admitted_disconnected,
            max_payload_bytes: chappe::ipc::MAX_PAYLOAD_BYTES as u64,
            max_in_flight_payload_bytes: chappe::ipc::MAX_PAYLOAD_BYTES as u64,
            write_failures_total: stats.write_failures,
        }),
    }
}

/// TCP-connect probe for the gateway HTTP port. Returns the measured
/// connect latency, or `None` when unreachable: unknown, never zero.
fn probe_gateway_health() -> Option<Duration> {
    let addr: SocketAddr = std::env::var("MARENGO_GATEWAY_HTTP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            "127.0.0.1:8080"
                .parse()
                .unwrap_or(SocketAddr::from(([127, 0, 0, 1], 8080)))
        });
    let start = Instant::now();
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).ok()?;
    Some(start.elapsed())
}
