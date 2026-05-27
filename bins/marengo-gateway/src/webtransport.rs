use std::path::{Path, PathBuf};
use std::sync::Arc;

use armee_proto::prost::Message;
use armee_proto::GatewaySubscribe;
use bytes::{BufMut, BytesMut};
use tracing::{info, warn};
use web_transport_quinn::{ServerBuilder, Session};

use crate::state::{filter_topics, SharedState};

const MAX_FRAME: usize = 4 * 1024 * 1024;

/// TLS material for WebTransport (QUIC) and `/tls/fingerprint` for browsers.
pub struct TlsMaterial {
    pub certs: Vec<rustls::pki_types::CertificateDer<'static>>,
    pub key: rustls::pki_types::PrivateKeyDer<'static>,
    pub cert_sha256_base64: String,
}

pub async fn run_webtransport(
    state: SharedState,
    bind_addr: std::net::SocketAddr,
    tls: TlsMaterial,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    state.set_tls_cert_sha256_base64(tls.cert_sha256_base64.clone());
    let (cert, key) = (tls.certs, tls.key);
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

pub fn load_or_generate_tls(
    cert_path: Option<PathBuf>,
    key_path: Option<PathBuf>,
) -> Result<TlsMaterial, Box<dyn std::error::Error + Send + Sync>> {
    let (cert_file, key_file) = match (cert_path, key_path) {
        (Some(c), Some(k)) => (c, k),
        (None, None) => {
            let dir = default_tls_dir();
            (dir.join("cert.pem"), dir.join("key.pem"))
        }
        _ => return Err("both --tls-cert and --tls-key are required when overriding paths".into()),
    };

    if cert_file.is_file() && key_file.is_file() {
        if pem_valid_for_webtransport(&cert_file)? {
            return tls_material_from_pem_files(&cert_file, &key_file);
        }
        tracing::warn!(
            cert = %cert_file.display(),
            "replacing TLS cert (WebTransport requires ECDSA P-256 and <=14 day validity)"
        );
    }

    persist_bench_tls_pem(&cert_file, &key_file)?;
    tls_material_from_pem_files(&cert_file, &key_file)
}

/// Chrome WebTransport `serverCertificateHashes` only accepts short-lived ECDSA certs.
fn pem_valid_for_webtransport(
    cert_file: &Path,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let pem = std::fs::read(cert_file)?;
    let mut pem_slice = pem.as_slice();
    let mut certs = rustls_pemfile::certs(&mut pem_slice);
    let first = match certs.next() {
        Some(Ok(c)) => c,
        _ => return Ok(false),
    };
    use x509_parser::prelude::FromDer;
    let (_rest, cert) = x509_parser::certificate::X509Certificate::from_der(first.as_ref())
        .map_err(|e| format!("x509 parse: {e:?}"))?;
    let not_before = cert.validity().not_before.to_datetime();
    let not_after = cert.validity().not_after.to_datetime();
    let now = time::OffsetDateTime::now_utc();
    let lifetime = not_after - not_before;
    let valid_now = now >= not_before && now < not_after;
    let short_lived = lifetime <= time::Duration::days(14);
    let has_server_auth_eku = cert.extensions().iter().any(|ext| {
        matches!(
            ext.parsed_extension(),
            x509_parser::extensions::ParsedExtension::ExtendedKeyUsage(eku) if eku.server_auth
        )
    });
    Ok(valid_now && short_lived && has_server_auth_eku)
}

fn bench_san_names() -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut names = vec![
        "localhost".to_string(),
        "marengo.local".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    if let Ok(extra) = std::env::var("MARENGO_GATEWAY_TLS_EXTRA_SAN") {
        for entry in extra.split(',') {
            let trimmed = entry.trim();
            if !trimmed.is_empty() {
                names.push(trimmed.to_string());
            }
        }
    }
    Ok(names)
}

fn persist_bench_tls_pem(
    cert_file: &Path,
    key_file: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use rcgen::{
        CertificateParams, DnType, ExtendedKeyUsagePurpose, KeyPair, KeyUsagePurpose,
        PKCS_ECDSA_P256_SHA256,
    };
    let mut params = CertificateParams::new(bench_san_names()?)?;
    let now = time::OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + time::Duration::days(13);
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params
        .distinguished_name
        .push(DnType::CommonName, "marengo-gateway");
    let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
    let cert = params.self_signed(&key_pair)?;
    if let Some(parent) = cert_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(cert_file, cert.pem())?;
    std::fs::write(key_file, key_pair.serialize_pem())?;
    tracing::info!(
        cert = %cert_file.display(),
        "generated bench TLS certificate (13-day validity, ECDSA P-256)"
    );
    Ok(())
}

fn default_tls_dir() -> PathBuf {
    std::env::var("MARENGO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("var/gateway/tls")
}

/// PEM paths used by WebTransport and the optional HTTPS Consul listener.
pub fn resolve_tls_pem_paths(
    cert_path: Option<PathBuf>,
    key_path: Option<PathBuf>,
) -> (PathBuf, PathBuf) {
    match (cert_path, key_path) {
        (Some(c), Some(k)) => (c, k),
        _ => {
            let dir = default_tls_dir();
            (dir.join("cert.pem"), dir.join("key.pem"))
        }
    }
}

fn tls_material_from_pem_files(
    cert_file: &Path,
    key_file: &Path,
) -> Result<TlsMaterial, Box<dyn std::error::Error + Send + Sync>> {
    let cert_pem = std::fs::read(cert_file)?;
    let key_pem = std::fs::read(key_file)?;
    let certs = rustls_pemfile::certs(&mut cert_pem.as_slice()).collect::<Result<Vec<_>, _>>()?;
    let key = rustls_pemfile::private_key(&mut key_pem.as_slice())?.ok_or("missing private key")?;
    let first = certs.first().ok_or("no certificate in PEM")?;
    let cert_sha256 = cert_sha256_from_der(first.as_ref());
    let cert_sha256_base64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, cert_sha256);
    Ok(TlsMaterial {
        certs,
        key,
        cert_sha256_base64,
    })
}

/// SHA-256 of DER-encoded certificate (WebTransport `serverCertificateHashes`).
fn cert_sha256_from_der(cert_der: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(cert_der).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cert_hash_is_stable_for_der_roundtrip() {
        let generated = rcgen::generate_simple_self_signed(vec!["localhost".into()]).expect("cert");
        let der = generated.cert.der();
        let a = cert_sha256_from_der(der);
        let b = cert_sha256_from_der(der);
        assert_eq!(a, b);
    }
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
