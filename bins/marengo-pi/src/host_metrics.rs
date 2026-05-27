//! 1 Hz HostMetrics publisher for Consul host cards.

use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use armee_proto::HostNodeRole;
use chappe::Bus;
use marengo_host_metrics::{host_metrics_topic, sample, ChappeHealthInput, SampleState};
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
    ChappeHealthInput {
        ipc_connected: chappe.ipc_configured(),
        gateway_reachable: probe_gateway_health(),
        last_publish_age_ms: now_ms.saturating_sub(last),
        gateway_rtt_ms: 0.0,
    }
}

fn probe_gateway_health() -> bool {
    let addr: SocketAddr = std::env::var("MARENGO_GATEWAY_HTTP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            "127.0.0.1:8080"
                .parse()
                .unwrap_or(SocketAddr::from(([127, 0, 0, 1], 8080)))
        });
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}
