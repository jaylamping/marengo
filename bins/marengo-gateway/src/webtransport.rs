use std::sync::Arc;

use armee_proto::prost::Message;
use armee_proto::GatewaySubscribe;
use bytes::{BufMut, BytesMut};
use tracing::{info, warn};
use web_transport_quinn::{ServerBuilder, Session};

use crate::state::{filter_topics, SharedState};

const MAX_FRAME: usize = 4 * 1024 * 1024;

pub async fn run_webtransport(
    state: SharedState,
    bind_addr: std::net::SocketAddr,
    cert_path: Option<std::path::PathBuf>,
    key_path: Option<std::path::PathBuf>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (cert, key) = load_or_generate_tls(cert_path, key_path)?;
    let mut server = ServerBuilder::new()
        .with_addr(bind_addr)
        .with_certificate(cert, key)?;
    info!(%bind_addr, "WebTransport listening");
    loop {
        let Some(request) = server.accept().await else {
            continue;
        };
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            match request.ok().await {
                Ok(session) => {
                    if let Err(e) = handle_session(session, st).await {
                        warn!(error = %e, "webtransport session failed");
                    }
                }
                Err(e) => warn!(error = %e, "webtransport handshake failed"),
            }
        });
    }
}

async fn handle_session(
    session: Session,
    state: SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut send, mut recv) = session
        .accept_bi()
        .await
        .map_err(|e| format!("accept_bi: {e}"))?;

    let subscribe_bytes = read_length_prefixed(&mut recv).await?;
    let subscribe = GatewaySubscribe::decode(subscribe_bytes.as_slice())
        .map_err(|e| format!("subscribe decode: {e}"))?;
    let topics = filter_topics(&subscribe.topics);
    if topics.is_empty() {
        return Err("no allowed topics in subscribe".into());
    }
    info!(?topics, "WebTransport subscribed");

    let mut rx = state.subscribe_envelopes();
    loop {
        tokio::select! {
            msg = rx.recv() => {
                let Ok((topic, payload)) = msg else {
                    continue;
                };
                if !topics.iter().any(|t| t == &topic) {
                    continue;
                }
                let envelope = armee_proto::Envelope::decode(payload.as_slice())
                    .map_err(|e| format!("envelope: {e}"))?;
                let out = envelope.encode_to_vec();
                write_length_prefixed(&mut send, &out).await?;
            }
            chunk = recv.read_chunk(MAX_FRAME, true) => {
                if chunk?.is_none() {
                    break;
                }
            }
        }
    }
    Ok(())
}

async fn read_length_prefixed(
    recv: &mut web_transport_quinn::RecvStream,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME {
        return Err("frame too large".into());
    }
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn write_length_prefixed(
    send: &mut web_transport_quinn::SendStream,
    payload: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buf = BytesMut::with_capacity(4 + payload.len());
    buf.put_u32_le(payload.len() as u32);
    buf.extend_from_slice(payload);
    send.write_chunk(buf.freeze()).await?;
    Ok(())
}

fn load_or_generate_tls(
    cert_path: Option<std::path::PathBuf>,
    key_path: Option<std::path::PathBuf>,
) -> Result<
    (
        Vec<rustls::pki_types::CertificateDer<'static>>,
        rustls::pki_types::PrivateKeyDer<'static>,
    ),
    Box<dyn std::error::Error + Send + Sync>,
> {
    if let (Some(cert), Some(key)) = (cert_path, key_path) {
        let cert_pem = std::fs::read(cert)?;
        let key_pem = std::fs::read(key)?;
        let certs =
            rustls_pemfile::certs(&mut cert_pem.as_slice()).collect::<Result<Vec<_>, _>>()?;
        let key =
            rustls_pemfile::private_key(&mut key_pem.as_slice())?.ok_or("missing private key")?;
        return Ok((certs, key));
    }

    let cert = rcgen::generate_simple_self_signed(vec![
        "localhost".into(),
        "marengo.local".into(),
        "127.0.0.1".into(),
    ])?;
    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert.der().to_vec());
    let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(cert.key_pair.serialize_der().into());
    Ok((vec![cert_der], key_der))
}

/// Demo publisher for local testing without `marengo-pi`.
pub fn spawn_demo_publisher(state: SharedState) {
    use armee_proto::{Heartbeat, JointState, OperationalMode, RobotState, SafetyState};
    tokio::spawn(async move {
        let mut t = 0u64;
        loop {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let angle = (t as f64) * 0.02;
            let robot = RobotState {
                timestamp_ms: ts,
                joints: vec![JointState {
                    name: "left_shoulder_pitch".to_string(),
                    position: angle.sin() * 0.5,
                    velocity: angle.cos() * 0.1,
                    effort: 0.0,
                }],
            };
            let safety = SafetyState {
                timestamp_ms: ts,
                mode: OperationalMode::Ready as i32,
                hardware_estop_asserted: false,
                software_estop_latched: false,
                active_faults: vec![],
            };
            let hb = Heartbeat {
                timestamp_ms: ts,
                node_id: "demo".to_string(),
            };
            for (topic, msg, type_name) in [
                (
                    crate::state::TOPIC_STATE,
                    robot.encode_to_vec(),
                    "marengo.v1.RobotState",
                ),
                (
                    crate::state::TOPIC_SAFETY,
                    safety.encode_to_vec(),
                    "marengo.v1.SafetyState",
                ),
                (
                    crate::state::TOPIC_HEARTBEAT,
                    hb.encode_to_vec(),
                    "marengo.v1.Heartbeat",
                ),
            ] {
                let envelope = armee_proto::Envelope {
                    timestamp_ms: ts,
                    source_node: "demo".into(),
                    message_type: type_name.into(),
                    payload: msg,
                };
                state.ingest_runtime_frame(topic.to_string(), envelope.encode_to_vec());
            }
            t += 1;
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    });
}
