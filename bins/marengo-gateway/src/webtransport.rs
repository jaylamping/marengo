use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use armee_proto::prost::Message;
use armee_proto::{GatewaySubscribe, GatewaySubscriptionAdmission};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use tracing::{debug, info, warn};
use web_transport_quinn::{Server, ServerBuilder, Session};

use crate::framing::{self, MAX_FRAME};
use crate::state::{filter_topics, SharedState};

#[cfg(test)]
#[path = "webtransport_access_tests.rs"]
mod access_tests;

const SUBSCRIPTION_MAX_BYTES: usize = 16 * 1024;
const SUBSCRIPTION_TIMEOUT: Duration = Duration::from_secs(5);
const ADMISSION_WRITE_TIMEOUT: Duration = Duration::from_secs(1);
const SESSION_CAPACITY: usize = 64;

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
    let (cert, key) = (tls.certs, tls.key);
    let server = ServerBuilder::new()
        .with_addr(bind_addr)
        .with_certificate(cert, key)?;
    info!(%bind_addr, "WebTransport listening");
    serve_webtransport(server, state).await
}

async fn serve_webtransport(
    mut server: Server,
    state: SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let capacity = Arc::new(tokio::sync::Semaphore::new(SESSION_CAPACITY));
    let mut sessions = tokio::task::JoinSet::new();
    loop {
        let request = tokio::select! {
            request = server.accept() => match request {
                Some(request) => request,
                None => return Ok(()),
            },
            _ = sessions.join_next(), if !sessions.is_empty() => continue,
        };
        let Ok(permit) = Arc::clone(&capacity).try_acquire_owned() else {
            // Refusal work is bounded too; do not spawn unlimited rejected sessions.
            let _ = tokio::time::timeout(
                ADMISSION_WRITE_TIMEOUT,
                request.reject(StatusCode::SERVICE_UNAVAILABLE),
            )
            .await;
            continue;
        };
        let st = Arc::clone(&state);
        sessions.spawn(async move {
            let _permit = permit;
            let mut headers = request.headers.clone();
            // HTTP/3 carries authority separately from raw headers. The same policy
            // uses that parsed CONNECT authority, never an Origin-supplied host.
            let authority = request
                .url
                .as_str()
                .strip_prefix("https://")
                .and_then(|url| url.split('/').next())
                .and_then(|authority| HeaderValue::from_str(authority).ok());
            if let Some(authority) = authority {
                headers.insert(header::HOST, authority);
            }
            let refusal = if request.url.path() != "/chappe"
                || request.url.query().is_some()
                || !request.url.username().is_empty()
                || request.url.password().is_some()
            {
                Some(StatusCode::BAD_REQUEST)
            } else {
                st.access.validate_origin(&headers).err()
            };
            if let Some(status) = refusal {
                let _ = tokio::time::timeout(ADMISSION_WRITE_TIMEOUT, request.reject(status)).await;
                return;
            }
            match tokio::time::timeout(SUBSCRIPTION_TIMEOUT, request.ok()).await {
                Ok(Ok(session)) => {
                    let closing = session.clone();
                    if let Err(e) = handle_session(session, st, headers).await {
                        closing.close(1, b"subscription refused");
                        warn!(error = %e, "webtransport session failed");
                    }
                }
                Ok(Err(e)) => warn!(error = %e, "webtransport handshake failed"),
                Err(_) => warn!("webtransport handshake timed out"),
            }
        });
    }
}

async fn handle_session(
    session: Session,
    state: SharedState,
    mut headers: HeaderMap,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut send, mut recv, subscribe_bytes) = tokio::time::timeout(SUBSCRIPTION_TIMEOUT, async {
        let (send, mut recv) = session.accept_bi().await?;
        let bytes = framing::read_length_prefixed_quinn(&mut recv, SUBSCRIPTION_MAX_BYTES).await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>((send, recv, bytes))
    })
    .await
    .map_err(|_| "subscription timed out")??;

    let admitted = GatewaySubscribe::decode(subscribe_bytes.as_slice())
        .map_err(|_| StatusCode::BAD_REQUEST)
        .and_then(|subscribe| {
            let topics = filter_topics(&subscribe.topics);
            if topics.is_empty() {
                return Err(StatusCode::BAD_REQUEST);
            }
            if topics
                .iter()
                .any(|topic| crate::http::sensitive_topic(topic))
            {
                if headers.contains_key(header::AUTHORIZATION)
                    || headers.contains_key("x-marengo-log-token")
                {
                    return Err(StatusCode::UNAUTHORIZED);
                }
                let value =
                    HeaderValue::from_str(&format!("Bearer {}", subscribe.runtime_credential))
                        .map_err(|_| StatusCode::UNAUTHORIZED)?;
                headers.insert(header::AUTHORIZATION, value);
                state
                    .access
                    .authorize(&headers, crate::access::Capability::SensitiveRead)?;
            }
            Ok(topics)
        });
    let admission = GatewaySubscriptionAdmission {
        status: admitted
            .as_ref()
            .map_or_else(|status| status.as_u16() as u32, |_| 200),
        topics: admitted.as_ref().cloned().unwrap_or_default(),
    };
    tokio::time::timeout(
        ADMISSION_WRITE_TIMEOUT,
        framing::write_length_prefixed_quinn(&mut send, &admission.encode_to_vec()),
    )
    .await
    .map_err(|_| "admission response timed out")??;
    let topics = match admitted {
        Ok(topics) => topics,
        Err(_) => {
            send.finish()?;
            // Give the refusal frame a bounded opportunity to reach the client.
            let _ = tokio::time::timeout(ADMISSION_WRITE_TIMEOUT, send.stopped()).await;
            session.close(1, b"subscription refused");
            return Ok(());
        }
    };
    debug!(?topics, "WebTransport subscribed");

    let mut rx = state.subscribe_envelopes();
    loop {
        tokio::select! {
            msg = framing::next_stream_envelope(&mut rx, &topics) => {
                let Some(out) = msg else { break; };
                framing::write_length_prefixed_quinn(&mut send, &out).await?;
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
    let mut certs = CertificateDer::pem_slice_iter(&pem);
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

#[cfg(unix)]
fn write_private_key(
    path: &Path,
    contents: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    file.write_all(contents)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private_key(
    path: &Path,
    contents: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    std::fs::write(path, contents)?;
    Ok(())
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
    write_private_key(key_file, key_pair.serialize_pem().as_bytes())?;
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
    let certs = CertificateDer::pem_slice_iter(&cert_pem).collect::<Result<Vec<_>, _>>()?;
    let key = PrivateKeyDer::from_pem_slice(&key_pem)?;
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

/// Demo publisher for local testing without `marengo-pi`.
pub fn spawn_demo_publisher(state: SharedState) {
    use armee_proto::{
        BuildInfo, ChappeHealth, ClockMetrics, CpuMetrics, Heartbeat, HostMetrics, HostNodeRole,
        JetsonPlatformMetrics, JointState, LoadMetrics, LogEvent, MemoryMetrics, OperationalMode,
        PiPlatformMetrics, RobotState, SafetyState, ThermalMetrics,
    };
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
                    temperature_c: 0.0,
                    fault: 0,
                    // Demo publisher: Unhomed (non-UNSPECIFIED) so Consul wire-gate opens.
                    homing_state: armee_proto::JointHomingState::Unhomed as i32,
                    drive_active: false,
                    out_of_limits: false,
                    // Synthesized live each tick: age zero is true, not a default.
                    sample_age_ms: 0,
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
            let log = LogEvent {
                timestamp_ms: ts,
                level: "info".to_string(),
                target: "marengo_gateway::demo".to_string(),
                message: format!("demo tick {t}"),
                session_id: String::new(),
                fields_json: String::new(),
            };
            let host_pi = HostMetrics {
                timestamp_ms: ts,
                hostname: "demo-pi".to_string(),
                node_role: HostNodeRole::Pi as i32,
                uptime_sec: t,
                kernel_version: "demo".to_string(),
                os_pretty_name: "Marengo Demo".to_string(),
                build: Some(BuildInfo {
                    deploy_rev: "demo".to_string(),
                    git_sha: "demo".to_string(),
                    semver: env!("CARGO_PKG_VERSION").to_string(),
                }),
                cpu: Some(CpuMetrics {
                    sample_valid: true,
                    usage_percent: 12.0 + (angle.sin() * 10.0),
                    core_count: 4,
                    ..Default::default()
                }),
                memory: Some(MemoryMetrics {
                    total_bytes: 8 * 1024 * 1024 * 1024,
                    used_bytes: 2 * 1024 * 1024 * 1024,
                    available_bytes: 6 * 1024 * 1024 * 1024,
                    ..Default::default()
                }),
                load: Some(LoadMetrics {
                    load_1m: 0.42,
                    ..Default::default()
                }),
                thermal: Some(ThermalMetrics {
                    cpu_celsius: 48.0,
                    ..Default::default()
                }),
                chappe: Some(ChappeHealth {
                    ipc_connected: true,
                    gateway_reachable: true,
                    ..Default::default()
                }),
                clock: Some(ClockMetrics {
                    sync_source: "demo".to_string(),
                    synchronized: true,
                    ..Default::default()
                }),
                platform: Some(armee_proto::host_metrics::Platform::Pi(PiPlatformMetrics {
                    throttled_now: false,
                    ..Default::default()
                })),
                ..Default::default()
            };
            let host_jetson = HostMetrics {
                timestamp_ms: ts,
                hostname: "demo-jetson".to_string(),
                node_role: HostNodeRole::Jetson as i32,
                uptime_sec: t,
                build: Some(BuildInfo {
                    deploy_rev: "demo".to_string(),
                    git_sha: "demo".to_string(),
                    semver: env!("CARGO_PKG_VERSION").to_string(),
                }),
                cpu: Some(CpuMetrics {
                    sample_valid: true,
                    usage_percent: 18.0,
                    core_count: 8,
                    ..Default::default()
                }),
                memory: Some(MemoryMetrics {
                    total_bytes: 16 * 1024 * 1024 * 1024,
                    used_bytes: 6 * 1024 * 1024 * 1024,
                    available_bytes: 10 * 1024 * 1024 * 1024,
                    ..Default::default()
                }),
                load: Some(LoadMetrics {
                    load_1m: 0.88,
                    ..Default::default()
                }),
                thermal: Some(ThermalMetrics {
                    cpu_celsius: 44.0,
                    gpu_celsius: 51.0,
                    ..Default::default()
                }),
                chappe: Some(ChappeHealth {
                    ipc_connected: true,
                    gateway_reachable: true,
                    ..Default::default()
                }),
                clock: Some(ClockMetrics {
                    sync_source: "demo".to_string(),
                    synchronized: true,
                    ..Default::default()
                }),
                platform: Some(armee_proto::host_metrics::Platform::Jetson(
                    JetsonPlatformMetrics {
                        jetson_model: "Demo Orin".to_string(),
                        power_mode: "MAXN".to_string(),
                        gpu_usage_percent: 55.0,
                        chappe_connected: true,
                        chappe_rtt_ms: 1.2,
                        ..Default::default()
                    },
                )),
                ..Default::default()
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
                (
                    crate::state::TOPIC_LOGS,
                    log.encode_to_vec(),
                    "marengo.v1.LogEvent",
                ),
                (
                    crate::state::TOPIC_HOST_METRICS_PI,
                    host_pi.encode_to_vec(),
                    "marengo.v1.HostMetrics",
                ),
                (
                    crate::state::TOPIC_HOST_METRICS_JETSON,
                    host_jetson.encode_to_vec(),
                    "marengo.v1.HostMetrics",
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn tls_material_retains_certificate_chain_and_matching_key() {
        use sha2::{Digest, Sha256};
        let dir = tempfile::tempdir().expect("fixture directory");
        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        persist_bench_tls_pem(&cert_path, &key_path).expect("certificate fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&key_path)
                    .expect("key metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        let first = CertificateDer::from_pem_slice(&std::fs::read(&cert_path).expect("PEM"))
            .expect("fixture DER");
        let second = rcgen::generate_simple_self_signed(vec!["chain.fixture".into()])
            .expect("second certificate");
        let mut chain = std::fs::read_to_string(&cert_path).expect("certificate text");
        chain.push_str(&second.cert.pem());
        std::fs::write(&cert_path, chain).expect("chain fixture");
        let material = load_or_generate_tls(Some(cert_path), Some(key_path)).expect("load chain");
        assert_eq!(material.certs.len(), 2);
        assert_eq!(material.certs[0], first);
        assert_eq!(material.certs[1].as_ref(), second.cert.der().as_ref());
        let expected_hash = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            Sha256::digest(first.as_ref()),
        );
        assert_eq!(material.cert_sha256_base64, expected_hash);
        let config = rustls::ServerConfig::builder_with_provider(std::sync::Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("TLS versions")
        .with_no_client_auth()
        .with_single_cert(material.certs, material.key);
        assert!(
            config.is_ok(),
            "parsed private key must match the leaf certificate"
        );
    }

    #[test]
    fn tls_material_rejects_empty_or_malformed_private_key() {
        for key_pem in [
            "",
            "-----BEGIN PRIVATE KEY-----\n!invalid!\n-----END PRIVATE KEY-----\n",
        ] {
            let dir = tempfile::tempdir().expect("fixture directory");
            let cert_path = dir.path().join("cert.pem");
            let key_path = dir.path().join("key.pem");
            persist_bench_tls_pem(&cert_path, &key_path).expect("certificate fixture");
            let before = std::fs::read(&cert_path).expect("certificate bytes");
            std::fs::write(&key_path, key_pem).expect("bad key fixture");
            assert!(load_or_generate_tls(Some(cert_path.clone()), Some(key_path)).is_err());
            assert_eq!(std::fs::read(cert_path).expect("preserved cert"), before);
        }
    }

    #[test]
    fn tls_material_rejects_malformed_certificate_chain() {
        let dir = tempfile::tempdir().expect("fixture directory");
        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        persist_bench_tls_pem(&cert_path, &key_path).expect("certificate fixture");
        let mut chain = std::fs::read_to_string(&cert_path).expect("certificate text");
        chain.push_str("-----BEGIN CERTIFICATE-----\n!invalid!\n-----END CERTIFICATE-----\n");
        std::fs::write(&cert_path, chain).expect("bad chain fixture");
        assert!(load_or_generate_tls(Some(cert_path), Some(key_path)).is_err());
    }
}
