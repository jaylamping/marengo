//! Operator gateway: HTTP CRUD snapshots/commands + WebTransport telemetry streams.

mod http;
mod state;
mod webtransport;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use chappe::ipc::{default_socket_path, socket_path_from_env, IpcListener};
use chappe::Bus;
use marengo_support::init_tracing;
use tracing::info;

#[derive(Debug)]
struct Args {
    http_addr: SocketAddr,
    wt_addr: SocketAddr,
    socket_path: PathBuf,
    demo: bool,
    cert_path: Option<PathBuf>,
    key_path: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut http_addr: SocketAddr = "127.0.0.1:8080"
        .parse()
        .map_err(|e| format!("http addr: {e}"))?;
    let mut wt_addr: SocketAddr = "127.0.0.1:8443"
        .parse()
        .map_err(|e| format!("wt addr: {e}"))?;
    let mut socket_path = socket_path_from_env().unwrap_or_else(default_socket_path);
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
            "--wt-listen" => {
                let raw = args.next().ok_or("--wt-listen needs host:port")?;
                wt_addr = raw.parse().map_err(|e| format!("wt listen: {e}"))?;
            }
            "--chappe-socket" => {
                socket_path = PathBuf::from(args.next().ok_or("--chappe-socket needs path")?);
            }
            "--demo" => demo = true,
            "--tls-cert" => cert_path = Some(PathBuf::from(args.next().ok_or("cert path")?)),
            "--tls-key" => key_path = Some(PathBuf::from(args.next().ok_or("key path")?)),
            "--help" | "-h" => {
                eprintln!(
                    "marengo-gateway [--http-listen HOST:PORT] [--wt-listen HOST:PORT] \
                     [--chappe-socket PATH] [--demo] [--tls-cert PATH --tls-key PATH]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Args {
        http_addr,
        wt_addr,
        socket_path,
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
    init_tracing();
    let args = parse_args().map_err(|e| e.to_string())?;
    let bus = Arc::new(Bus::default());
    let state_holder: Arc<std::sync::Mutex<Option<state::SharedState>>> =
        Arc::new(std::sync::Mutex::new(None));

    let on_frame = {
        let holder = Arc::clone(&state_holder);
        Arc::new(move |topic: String, payload: Vec<u8>| {
            if let Ok(guard) = holder.lock() {
                if let Some(st) = guard.as_ref() {
                    st.ingest_runtime_frame(topic, payload);
                }
            }
        })
    };

    let ipc = IpcListener::spawn_server(args.socket_path.clone(), on_frame)?;
    let state: state::SharedState = Arc::new(state::AppState::new(Arc::clone(&bus)).with_ipc(ipc));
    if let Ok(mut guard) = state_holder.lock() {
        *guard = Some(Arc::clone(&state));
    }

    state::spawn_bus_fanout(Arc::clone(&state));

    if args.demo {
        info!("demo publisher enabled (no marengo-pi required)");
        webtransport::spawn_demo_publisher(Arc::clone(&state));
    }

    let tls = webtransport::load_or_generate_tls(args.cert_path, args.key_path)?;
    state.set_tls_cert_sha256_base64(tls.cert_sha256_base64.clone());

    let http_state = Arc::clone(&state);
    let http_addr = args.http_addr;
    tokio::spawn(async move {
        let app = http::router(http_state);
        match tokio::net::TcpListener::bind(http_addr).await {
            Ok(listener) => {
                info!(%http_addr, "HTTP listening");
                if let Err(e) = axum::serve(listener, app).await {
                    tracing::error!(error = %e, "http serve failed");
                }
            }
            Err(e) => tracing::error!(error = %e, %http_addr, "http bind failed"),
        }
    });

    webtransport::run_webtransport(state, args.wt_addr, tls).await?;
    Ok(())
}
