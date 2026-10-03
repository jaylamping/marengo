//! Operator gateway: HTTP CRUD snapshots/commands + WebTransport telemetry streams.

mod access;
mod action_ack;
mod actuator;
mod config;
mod deploy;
mod framing;
mod hardware;
mod http;
mod limit_patch;
mod logs;
mod ratelimit;
mod restart;
mod state;
mod webtransport;

#[cfg(test)]
mod gateway_access_public_test;

#[cfg(test)]
mod gateway_access_conformance_test;

#[cfg(test)]
mod command_hardening_test;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum_server::tls_rustls::RustlsConfig;
use chappe::ipc::{default_socket_path, socket_path_from_env, IpcListener};
use chappe::Bus;
use marengo_store::Store;
use tracing::info;

use crate::logs::LogServices;

enum IpcCallbackEvent {
    Frame(String, Vec<u8>),
    ConnectionChanged(bool),
}

#[derive(Default)]
struct IpcStateHolder {
    state: Option<state::SharedState>,
    pending: Vec<IpcCallbackEvent>,
}

impl IpcStateHolder {
    fn dispatch(&mut self, event: IpcCallbackEvent) {
        if let Some(state) = &self.state {
            match event {
                IpcCallbackEvent::Frame(topic, payload) => {
                    state.ingest_runtime_frame(topic, payload)
                }
                IpcCallbackEvent::ConnectionChanged(connected) => {
                    state.runtime_connection_changed(connected)
                }
            }
        } else {
            self.pending.push(event);
        }
    }

    fn attach(&mut self, state: state::SharedState) {
        self.state = Some(Arc::clone(&state));
        for event in self.pending.drain(..) {
            match event {
                IpcCallbackEvent::Frame(topic, payload) => {
                    state.ingest_runtime_frame(topic, payload)
                }
                IpcCallbackEvent::ConnectionChanged(connected) => {
                    state.runtime_connection_changed(connected)
                }
            }
        }
    }
}

#[derive(Debug)]
struct Args {
    http_addr: SocketAddr,
    https_addr: Option<SocketAddr>,
    wt_addr: SocketAddr,
    socket_path: PathBuf,
    web_root: Option<PathBuf>,
    demo: bool,
    cert_path: Option<PathBuf>,
    key_path: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut http_addr: SocketAddr = "127.0.0.1:8080"
        .parse()
        .map_err(|e| format!("http addr: {e}"))?;
    let mut https_addr: Option<SocketAddr> = None;
    let mut wt_addr: SocketAddr = "127.0.0.1:8443"
        .parse()
        .map_err(|e| format!("wt addr: {e}"))?;
    let mut socket_path = socket_path_from_env().unwrap_or_else(default_socket_path);
    let mut web_root: Option<PathBuf> = None;
    let mut demo = false;
    let mut cert_path = None;
    let mut key_path = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--http-listen" => {
                let raw = args.next().ok_or("--http-listen needs host:port")?;
                http_addr = raw.parse().map_err(|e| format!("http listen: {e}"))?;
            }
            "--https-listen" => {
                let raw = args.next().ok_or("--https-listen needs host:port")?;
                https_addr = Some(raw.parse().map_err(|e| format!("https listen: {e}"))?);
            }
            "--wt-listen" => {
                let raw = args.next().ok_or("--wt-listen needs host:port")?;
                wt_addr = raw.parse().map_err(|e| format!("wt listen: {e}"))?;
            }
            "--web-root" => {
                web_root = Some(PathBuf::from(
                    args.next().ok_or("--web-root needs directory path")?,
                ));
            }
            "--chappe-socket" => {
                socket_path = PathBuf::from(args.next().ok_or("--chappe-socket needs path")?);
            }
            "--demo" => demo = true,
            "--tls-cert" => cert_path = Some(PathBuf::from(args.next().ok_or("cert path")?)),
            "--tls-key" => key_path = Some(PathBuf::from(args.next().ok_or("key path")?)),
            "--help" | "-h" => {
                eprintln!(
                    "marengo-gateway [--http-listen HOST:PORT] [--https-listen HOST:PORT] \
                     [--wt-listen HOST:PORT] [--web-root DIR] [--chappe-socket PATH] [--demo] \
                     [--tls-cert PATH --tls-key PATH]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Args {
        http_addr,
        https_addr,
        wt_addr,
        socket_path,
        web_root,
        demo,
        cert_path,
        key_path,
    })
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("marengo-gateway: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let args = parse_args().map_err(|e| e.to_string())?;
    let bus = Arc::new(Bus::default());
    chappe::tracing_layer::init_subscriber(Some(Arc::clone(&bus)), "marengo-gateway");
    let state_holder = Arc::new(std::sync::Mutex::new(IpcStateHolder::default()));

    // `--demo` synthesizes traffic at 20 Hz for UI work without a runtime.
    // It must never write into the real Store: point it at an ephemeral
    // per-process directory so demo ticks cannot pollute operator logs,
    // sessions, or archive blobs.
    let demo_root: Option<std::path::PathBuf> = args.demo.then(|| {
        let dir = std::env::temp_dir().join(format!("marengo-gateway-demo-{}", std::process::id()));
        let _ = std::fs::create_dir_all(dir.join("var").join("log"));
        dir
    });
    if demo_root.is_some() {
        info!("demo publisher enabled (ephemeral store; no marengo-pi required)");
    }

    let logs = {
        let root = demo_root
            .clone()
            .unwrap_or_else(marengo_store::resolve_marengo_root);
        let db = demo_root
            .as_ref()
            .map(|dir| dir.join("marengo.db"))
            .unwrap_or_else(marengo_store::resolve_db_path);
        let config_dir = marengo_config::resolve_config_dir(&root);
        let candump = match marengo_candump::Candump::with_robstride_from_config_dir(&config_dir) {
            Ok(c) => {
                info!(
                    config_dir = %config_dir.display(),
                    "candump robstride enrichment enabled"
                );
                c
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    config_dir = %config_dir.display(),
                    "candump enrichment unavailable; using plain parser"
                );
                marengo_candump::Candump::plain()
            }
        };
        match Store::open_with_candump(db, root, candump) {
            Ok(store) => {
                info!("marengo-store opened");
                Some(LogServices::open(store))
            }
            Err(e) => {
                tracing::warn!(error = %e, "marengo-store unavailable; log persistence disabled");
                None
            }
        }
    };

    let on_frame = {
        let holder = Arc::clone(&state_holder);
        Arc::new(move |topic: String, payload: Vec<u8>| {
            if let Ok(mut holder) = holder.lock() {
                holder.dispatch(IpcCallbackEvent::Frame(topic, payload));
            }
        })
    };

    let on_connection_change = {
        let holder = Arc::clone(&state_holder);
        Arc::new(move |connected: bool| {
            if let Ok(mut holder) = holder.lock() {
                holder.dispatch(IpcCallbackEvent::ConnectionChanged(connected));
            }
        })
    };
    let ipc = IpcListener::spawn_server_with_lifecycle(
        args.socket_path.clone(),
        on_frame,
        on_connection_change,
    )?;
    let command_joints = match marengo_config::load_command_joint_allowlist() {
        Ok(allowlist) => {
            let joints: Vec<_> = allowlist.iter().collect();
            info!(?joints, "loaded actuator command joint allowlist");
            allowlist
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "failed to load command joint allowlist; actuator commands will be rejected"
            );
            marengo_config::CommandJointAllowlist::empty()
        }
    };
    let mut app_state = state::AppState::new_with_https_port(
        Arc::clone(&bus),
        args.https_addr.map(|address| address.port()),
    )
    .with_command_joints(command_joints)
    .with_ipc(ipc);
    if let Some(log_services) = logs {
        app_state = app_state.with_logs(log_services);
    }
    let state: state::SharedState = Arc::new(app_state);
    if let Ok(mut holder) = state_holder.lock() {
        holder.attach(Arc::clone(&state));
    }

    state::spawn_bus_fanout(Arc::clone(&state));
    marengo_deploy::init_upstream_cache_from_disk();

    if args.demo {
        webtransport::spawn_demo_publisher(Arc::clone(&state));
    }

    let tls = webtransport::load_or_generate_tls(args.cert_path.clone(), args.key_path.clone())?;
    state.set_tls_cert_sha256_base64(tls.cert_sha256_base64.clone());

    let (cert_pem, key_pem) = webtransport::resolve_tls_pem_paths(args.cert_path, args.key_path);

    let http_state = Arc::clone(&state);
    let http_listener_state = Arc::clone(&state);
    let http_addr = args.http_addr;
    tokio::spawn(async move {
        let app = http::router(http_state, None);
        match tokio::net::TcpListener::bind(http_addr).await {
            Ok(listener) => {
                http_listener_state.set_http_listener(true);
                info!(%http_addr, "HTTP listening");
                if let Err(e) = axum::serve(listener, app).await {
                    tracing::error!(error = %e, "http serve failed");
                }
                http_listener_state.set_http_listener(false);
            }
            Err(e) => {
                http_listener_state.set_http_listener(false);
                tracing::error!(error = %e, %http_addr, "http bind failed");
            }
        }
    });

    if let Some(https_addr) = args.https_addr {
        let web_root = args
            .web_root
            .clone()
            .ok_or("--https-listen requires --web-root")?;
        if !web_root.is_dir() {
            return Err(format!(
                "web root not found or not a directory: {}",
                web_root.display()
            )
            .into());
        }
        let https_state = Arc::clone(&state);
        let https_listener_state = Arc::clone(&state);
        let rustls = RustlsConfig::from_pem_file(&cert_pem, &key_pem).await?;
        tokio::spawn(async move {
            let app = http::router(https_state, Some(web_root.as_path()));
            let listener = match tokio::net::TcpListener::bind(https_addr).await {
                Ok(listener) => listener,
                Err(error) => {
                    https_listener_state.set_https_listener(false);
                    tracing::error!(%error, %https_addr, "https bind failed");
                    return;
                }
            };
            let listener = match listener.into_std() {
                Ok(listener) => listener,
                Err(error) => {
                    https_listener_state.set_https_listener(false);
                    tracing::error!(%error, %https_addr, "https listener conversion failed");
                    return;
                }
            };
            let server = match axum_server::from_tcp_rustls(listener, rustls) {
                Ok(server) => server,
                Err(error) => {
                    https_listener_state.set_https_listener(false);
                    tracing::error!(%error, %https_addr, "https server construction failed");
                    return;
                }
            };
            https_listener_state.set_https_listener(true);
            info!(%https_addr, root = %web_root.display(), "HTTPS listening (Consul UI)");
            if let Err(error) = server.serve(app.into_make_service()).await {
                tracing::error!(%error, "https serve failed");
            }
            https_listener_state.set_https_listener(false);
        });
    }

    webtransport::run_webtransport(state, args.wt_addr, tls).await?;
    Ok(())
}

#[cfg(test)]
mod ipc_bootstrap_tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use armee_proto::prost::Message;

    #[test]
    fn callbacks_before_state_installation_are_replayed_in_order() {
        let bus = Arc::new(Bus::default());
        let mut holder = IpcStateHolder::default();
        holder.dispatch(IpcCallbackEvent::ConnectionChanged(true));
        let state_frame = armee_proto::Envelope {
            timestamp_ms: 1,
            source_node: "test-pi".into(),
            message_type: "marengo.v1.RobotState".into(),
            payload: armee_proto::RobotState {
                timestamp_ms: 42,
                joints: vec![],
            }
            .encode_to_vec(),
        }
        .encode_to_vec();
        holder.dispatch(IpcCallbackEvent::Frame(
            state::TOPIC_STATE.into(),
            state_frame,
        ));

        let state = Arc::new(state::AppState::new(bus));
        holder.attach(Arc::clone(&state));
        assert!(state.runtime_connected());
        assert_eq!(
            state
                .snapshot_robot_state()
                .expect("first IPC observation")
                .timestamp_ms,
            42
        );
    }
}
