//! Actual loopback QUIC admission and framing; certificate pinned to the fixture.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;
use crate::access::{AccessPolicy, Capability};
use crate::state::AppState;
use armee_proto::{Envelope, Heartbeat, LogEvent};

const TOKEN: &str = "isolated-quic-read-fixture";

/// Accepts exactly the fixture certificate, as the library's
/// `with_server_certificates` does.
#[derive(Debug)]
struct PinnedCertificate {
    cert: Vec<u8>,
    provider: web_transport_quinn::crypto::Provider,
}

impl rustls::client::danger::ServerCertVerifier for PinnedCertificate {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        if end_entity.as_ref() == self.cert.as_slice() {
            return Ok(rustls::client::danger::ServerCertVerified::assertion());
        }
        Err(rustls::Error::InvalidCertificate(
            rustls::CertificateError::UnknownIssuer,
        ))
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// A WebTransport client pinned to `cert` on an IPv4 loopback endpoint. The
/// library's `ClientBuilder` binds `[::]:0` and panics on hosts without IPv6.
fn ipv4_pinned_client(cert: &rustls::pki_types::CertificateDer<'_>) -> web_transport_quinn::Client {
    use web_transport_quinn::quinn;

    let provider = web_transport_quinn::crypto::default_provider();
    let mut crypto = rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS 1.3")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedCertificate {
            cert: cert.to_vec(),
            provider,
        }))
        .with_no_client_auth();
    crypto.alpn_protocols = vec![web_transport_quinn::ALPN.as_bytes().to_vec()];
    let crypto = quinn::crypto::rustls::QuicClientConfig::try_from(crypto).expect("QUIC TLS");
    let endpoint = quinn::Endpoint::client("127.0.0.1:0".parse().expect("IPv4 loopback"))
        .expect("IPv4 client endpoint");
    web_transport_quinn::Client::new(endpoint, quinn::ClientConfig::new(Arc::new(crypto)))
}

struct Fixture {
    server: tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    client: web_transport_quinn::Client,
    state: SharedState,
    url: url::Url,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Aborting the server drops its owned JoinSet and aborts every session.
        self.server.abort();
    }
}

impl Fixture {
    fn new(policy: AccessPolicy) -> Self {
        let dir = tempfile::tempdir().expect("owned TLS directory");
        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        persist_bench_tls_pem(&cert_path, &key_path).expect("actual bench TLS generator");
        let tls = load_or_generate_tls(Some(cert_path), Some(key_path)).expect("fixture TLS");
        let client = ipv4_pinned_client(tls.certs.first().expect("fixture certificate"));
        let server = ServerBuilder::new()
            .with_addr("127.0.0.1:0".parse().expect("owned address"))
            .with_certificate(tls.certs, tls.key)
            .expect("actual QUIC server");
        let addr = server.local_addr().expect("owned QUIC address");
        let state = Arc::new(AppState::new(Arc::new(chappe::Bus::default())).with_access(policy));
        let server = tokio::spawn(serve_webtransport(server, Arc::clone(&state)));
        Self {
            server,
            client,
            state,
            url: url::Url::parse(&format!("https://{addr}/chappe")).expect("owned URL"),
        }
    }

    async fn connect(
        &self,
        origin: Option<&str>,
    ) -> Result<Session, web_transport_quinn::ClientError> {
        let mut request = web_transport_quinn::proto::ConnectRequest::new(self.url.clone());
        if let Some(origin) = origin {
            request = request.with_header(
                header::ORIGIN,
                HeaderValue::from_str(origin).expect("fixture Origin"),
            );
        }
        tokio::time::timeout(Duration::from_secs(3), self.client.connect(request))
            .await
            .expect("bounded QUIC connect")
    }

    async fn subscribe(
        &self,
        session: &Session,
        topics: &[&str],
        credential: &str,
    ) -> (
        GatewaySubscriptionAdmission,
        web_transport_quinn::SendStream,
        web_transport_quinn::RecvStream,
    ) {
        let (mut send, mut recv) = session.open_bi().await.expect("client subscription stream");
        let subscribe = GatewaySubscribe {
            topics: topics.iter().map(|t| t.to_string()).collect(),
            runtime_credential: credential.into(),
        };
        framing::write_length_prefixed_quinn(&mut send, &subscribe.encode_to_vec())
            .await
            .expect("subscription");
        let bytes = tokio::time::timeout(
            Duration::from_secs(3),
            framing::read_length_prefixed_quinn(&mut recv, SUBSCRIPTION_MAX_BYTES),
        )
        .await
        .expect("bounded admission")
        .expect("admission frame");
        let admission =
            GatewaySubscriptionAdmission::decode(bytes.as_slice()).expect("typed admission");
        (admission, send, recv)
    }
}

#[tokio::test]
async fn actual_quic_rejects_missing_wrong_and_wrong_role_before_subscribing() {
    for (policy, correct_status) in [
        (AccessPolicy::default(), 401),
        (
            AccessPolicy::role_fixture(TOKEN, Capability::SensitiveRead).expect("read grant"),
            200,
        ),
        (
            AccessPolicy::role_fixture(TOKEN, Capability::Control).expect("control grant"),
            403,
        ),
    ] {
        let fixture = Fixture::new(policy);
        for credential in ["", "wrong-fixture", TOKEN] {
            let session = fixture.connect(None).await.expect("loopback session");
            let (admission, _send, _recv) = fixture
                .subscribe(
                    &session,
                    &["robot/heartbeat", "logs/structured"],
                    credential,
                )
                .await;
            assert_eq!(
                admission.status,
                if credential == TOKEN {
                    correct_status
                } else {
                    401
                }
            );
            if admission.status == 200 {
                assert_eq!(credential, TOKEN);
                assert_eq!(
                    admission.topics,
                    vec![
                        "robot/heartbeat",
                        "logs/structured",
                        "gateway/runtime_connection"
                    ]
                );
                assert_eq!(fixture.state.envelope_receiver_count(), 1);
            } else {
                assert!(matches!(admission.status, 401 | 403));
                assert!(admission.topics.is_empty());
                assert_eq!(fixture.state.envelope_receiver_count(), 0);
            }
            session.close(0, b"fixture complete");
            // The server's subscription is removed on peer close, before the next case.
            tokio::time::timeout(Duration::from_secs(2), async {
                while fixture.state.envelope_receiver_count() != 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("fixture subscription released");
        }
    }
}

#[tokio::test]
async fn actual_quic_enforces_browser_origin_and_allows_robot_https_listener_origin() {
    let policy = AccessPolicy::role_fixture(TOKEN, Capability::SensitiveRead)
        .expect("read grant")
        .with_robot_https_port(Some(8444));
    let fixture = Fixture::new(policy);
    for origin in [
        "https://foreign.fixture",
        "null",
        "http://127.0.0.1:8444",
        "https://127.0.0.1:9444",
    ] {
        assert!(
            fixture.connect(Some(origin)).await.is_err(),
            "refused Origin={origin}"
        );
        assert_eq!(fixture.state.envelope_receiver_count(), 0);
    }
    for origin in ["http://localhost:5173", "https://127.0.0.1:8444"] {
        let session = fixture
            .connect(Some(origin))
            .await
            .expect("approved browser session");
        let (admission, _send, _recv) = fixture
            .subscribe(&session, &["logs/structured"], TOKEN)
            .await;
        assert_eq!(admission.status, 200);
        session.close(0, b"fixture complete");
        tokio::time::timeout(Duration::from_secs(2), async {
            while fixture.state.envelope_receiver_count() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("subscriber released");
    }
}

#[tokio::test]
async fn actual_quic_public_and_authenticated_sensitive_streams_deliver_owned_envelopes() {
    for (policy, topic, credential, sensitive) in [
        (AccessPolicy::default(), "robot/heartbeat", "", false),
        (
            AccessPolicy::role_fixture(TOKEN, Capability::SensitiveRead).expect("read grant"),
            "logs/structured",
            TOKEN,
            true,
        ),
    ] {
        let fixture = Fixture::new(policy);
        let session = fixture.connect(None).await.expect("session");
        let (admission, _send, mut recv) = fixture.subscribe(&session, &[topic], credential).await;
        assert_eq!(admission.status, 200);
        assert_eq!(admission.topics, vec![topic, "gateway/runtime_connection"]);
        let (message_type, payload) = if sensitive {
            (
                "marengo.v1.LogEvent",
                LogEvent {
                    message: "owned fixture only".into(),
                    ..Default::default()
                }
                .encode_to_vec(),
            )
        } else {
            (
                "marengo.v1.Heartbeat",
                Heartbeat {
                    node_id: "owned fixture only".into(),
                    ..Default::default()
                }
                .encode_to_vec(),
            )
        };
        let envelope = Envelope {
            message_type: message_type.into(),
            payload,
            ..Default::default()
        };
        fixture
            .state
            .ingest_runtime_frame(topic.into(), envelope.encode_to_vec());
        let bytes = tokio::time::timeout(
            Duration::from_secs(2),
            framing::read_length_prefixed_quinn(&mut recv, MAX_FRAME),
        )
        .await
        .expect("bounded fixture frame")
        .expect("owned frame");
        assert_eq!(
            Envelope::decode(bytes.as_slice()).expect("envelope"),
            envelope
        );
        session.close(0, b"fixture complete");
    }
}

#[tokio::test]
async fn actual_quic_rejects_oversized_and_stalled_subscriptions_with_no_subscriber() {
    let fixture = Fixture::new(AccessPolicy::default());
    let session = fixture.connect(None).await.expect("session");
    let (mut send, _recv) = session.open_bi().await.expect("stream");
    send.write_all(&((SUBSCRIPTION_MAX_BYTES + 1) as u32).to_le_bytes())
        .await
        .expect("oversized length");
    tokio::time::timeout(Duration::from_secs(2), session.closed())
        .await
        .expect("oversized refusal bounded");
    assert_eq!(fixture.state.envelope_receiver_count(), 0);
    for half_body in [false, true] {
        let session = fixture
            .connect(None)
            .await
            .expect("stalled fixture session");
        let mut stream = None;
        if half_body {
            let (mut send, recv) = session.open_bi().await.expect("stream");
            send.write_all(&10u32.to_le_bytes())
                .await
                .expect("body length only");
            stream = Some((send, recv));
        }
        let started = std::time::Instant::now();
        tokio::time::timeout(
            SUBSCRIPTION_TIMEOUT + Duration::from_secs(2),
            session.closed(),
        )
        .await
        .expect("stalled admission bounded");
        assert!(started.elapsed() < SUBSCRIPTION_TIMEOUT + Duration::from_secs(2));
        assert_eq!(fixture.state.envelope_receiver_count(), 0);
        drop(stream);
    }
    // A stalled/refused client does not poison subsequent public subscriptions.
    let session = fixture.connect(None).await.expect("recovery session");
    let (admission, _send, _recv) = fixture.subscribe(&session, &["robot/heartbeat"], "").await;
    assert_eq!(admission.status, 200);
    session.close(0, b"fixture complete");
}

#[test]
fn tls_renewal_starts_a_day_before_expiry() {
    let dir = tempfile::tempdir().expect("owned TLS directory");
    let cert_path = dir.path().join("cert.pem");
    let key_path = dir.path().join("key.pem");
    let now = time::OffsetDateTime::now_utc();
    assert!(tls_needs_renewal(&cert_path, now), "missing certificate");
    persist_bench_tls_pem(&cert_path, &key_path).expect("13-day certificate");
    assert!(!tls_needs_renewal(&cert_path, now));
    assert!(tls_needs_renewal(
        &cert_path,
        now + time::Duration::days(12) + time::Duration::hours(1)
    ));
    persist_tls_pem_valid_for(&cert_path, &key_path, time::Duration::hours(12))
        .expect("short certificate");
    assert!(tls_needs_renewal(&cert_path, now));
}

#[tokio::test]
async fn renewed_certificate_reaches_https_new_quic_handshakes_and_fingerprint() {
    // The HTTPS listener's config uses the process provider, as main installs it.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let dir = tempfile::tempdir().expect("owned TLS directory");
    let cert_pem = dir.path().join("cert.pem");
    let key_pem = dir.path().join("key.pem");
    persist_tls_pem_valid_for(&cert_pem, &key_pem, time::Duration::hours(12))
        .expect("expiring certificate");
    let old = tls_material_from_pem_files(&cert_pem, &key_pem).expect("old TLS");
    let endpoint = quinn::Endpoint::server(
        quic_server_config(old.certs.clone(), old.key.clone_key()).expect("QUIC config"),
        "127.0.0.1:0".parse().expect("owned address"),
    )
    .expect("QUIC endpoint");
    let addr = endpoint.local_addr().expect("owned QUIC address");
    let state = Arc::new(AppState::new(Arc::new(chappe::Bus::default())));
    state.set_tls_cert_sha256_base64(old.cert_sha256_base64.clone());
    let server = tokio::spawn(serve_webtransport(
        Server::new(endpoint.clone()),
        Arc::clone(&state),
    ));
    let https = axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert_pem, &key_pem)
        .await
        .expect("HTTPS config");
    let renewal = TlsRenewal {
        cert_pem: cert_pem.clone(),
        key_pem: key_pem.clone(),
        https: Some(https),
    };
    let mut served = ServedTls {
        quic: old.cert_sha256_base64.clone(),
        https: Some(old.cert_sha256_base64.clone()),
    };

    renew_tls(&state, &endpoint, &renewal, &mut served)
        .await
        .expect("renewal");

    let new = tls_material_from_pem_files(&cert_pem, &key_pem).expect("new TLS");
    assert_ne!(new.cert_sha256_base64, old.cert_sha256_base64);
    assert_eq!(state.tls_cert_sha256_base64(), Some(new.cert_sha256_base64));
    assert!(!tls_needs_renewal(
        &cert_pem,
        time::OffsetDateTime::now_utc()
    ));
    let url = url::Url::parse(&format!("https://{addr}/chappe")).expect("owned URL");
    let connect = |cert: CertificateDer<'static>| {
        let request = web_transport_quinn::proto::ConnectRequest::new(url.clone());
        async move {
            let client = ipv4_pinned_client(&cert);
            tokio::time::timeout(Duration::from_secs(3), client.connect(request))
                .await
                .expect("bounded QUIC connect")
        }
    };
    assert!(
        connect(new.certs[0].clone()).await.is_ok(),
        "new handshakes present the renewed certificate"
    );
    assert!(
        connect(old.certs[0].clone()).await.is_err(),
        "the expiring certificate is no longer served"
    );
    let https = renewal.https.as_ref().expect("HTTPS config");
    assert!(https_presents(https, &new.certs[0]).await);
    assert!(!https_presents(https, &old.certs[0]).await);
    server.abort();
}

/// True when a TLS handshake against the HTTPS listener's current config
/// presents `cert`.
async fn https_presents(
    config: &axum_server::tls_rustls::RustlsConfig,
    cert: &CertificateDer<'static>,
) -> bool {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let acceptor = tokio_rustls::TlsAcceptor::from(config.get_inner());
    let server = tokio::spawn(async move { acceptor.accept(server_io).await.is_ok() });
    let provider = web_transport_quinn::crypto::default_provider();
    let client = rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .expect("TLS versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedCertificate {
            cert: cert.to_vec(),
            provider,
        }))
        .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(client));
    let name = rustls::pki_types::ServerName::try_from("localhost").expect("server name");
    let presented =
        tokio::time::timeout(Duration::from_secs(3), connector.connect(name, client_io))
            .await
            .expect("bounded TLS handshake")
            .is_ok();
    server.abort();
    presented
}

/// A renewal that wrote fresh PEM files but stopped before every listener took
/// them (a failed QUIC config build or HTTPS reload) is finished by the next
/// check, although the file alone no longer asks for renewal.
#[tokio::test]
async fn half_applied_renewal_is_finished_on_the_next_check() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let dir = tempfile::tempdir().expect("owned TLS directory");
    let cert_pem = dir.path().join("cert.pem");
    let key_pem = dir.path().join("key.pem");
    persist_bench_tls_pem(&cert_pem, &key_pem).expect("served certificate");
    let old = tls_material_from_pem_files(&cert_pem, &key_pem).expect("old TLS");
    let endpoint = quinn::Endpoint::server(
        quic_server_config(old.certs.clone(), old.key.clone_key()).expect("QUIC config"),
        "127.0.0.1:0".parse().expect("owned address"),
    )
    .expect("QUIC endpoint");
    let addr = endpoint.local_addr().expect("owned QUIC address");
    let state = Arc::new(AppState::new(Arc::new(chappe::Bus::default())));
    state.set_tls_cert_sha256_base64(old.cert_sha256_base64.clone());
    let server = tokio::spawn(serve_webtransport(
        Server::new(endpoint.clone()),
        Arc::clone(&state),
    ));
    let https = axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert_pem, &key_pem)
        .await
        .expect("HTTPS config");
    let renewal = TlsRenewal {
        cert_pem: cert_pem.clone(),
        key_pem: key_pem.clone(),
        https: Some(https.clone()),
    };
    let mut served = ServedTls {
        quic: old.cert_sha256_base64.clone(),
        https: Some(old.cert_sha256_base64.clone()),
    };
    // The renewal wrote new files, then failed before any listener took them.
    persist_bench_tls_pem(&cert_pem, &key_pem).expect("renewed certificate");
    let new = tls_material_from_pem_files(&cert_pem, &key_pem).expect("new TLS");
    let now = time::OffsetDateTime::now_utc();
    assert!(!tls_needs_renewal(&cert_pem, now), "the file is fresh");

    check_tls(&state, &endpoint, &renewal, &mut served, now).await;

    assert_eq!(
        state.tls_cert_sha256_base64(),
        Some(new.cert_sha256_base64.clone())
    );
    assert!(
        https_presents(&https, &new.certs[0]).await,
        "HTTPS took the renewal"
    );
    let url = url::Url::parse(&format!("https://{addr}/chappe")).expect("owned URL");
    let client = ipv4_pinned_client(&new.certs[0]);
    let request = web_transport_quinn::proto::ConnectRequest::new(url);
    assert!(
        tokio::time::timeout(Duration::from_secs(3), client.connect(request))
            .await
            .expect("bounded QUIC connect")
            .is_ok(),
        "QUIC took the renewal"
    );

    // QUIC and the fingerprint took a renewal, HTTPS did not: HTTPS catches up.
    persist_bench_tls_pem(&cert_pem, &key_pem).expect("second renewal");
    let newer = tls_material_from_pem_files(&cert_pem, &key_pem).expect("newer TLS");
    endpoint.set_server_config(Some(
        quic_server_config(newer.certs.clone(), newer.key.clone_key()).expect("QUIC config"),
    ));
    state.set_tls_cert_sha256_base64(newer.cert_sha256_base64.clone());
    served.quic.clone_from(&newer.cert_sha256_base64);
    check_tls(&state, &endpoint, &renewal, &mut served, now).await;
    assert!(
        https_presents(&https, &newer.certs[0]).await,
        "HTTPS caught up"
    );
    assert_eq!(served.https, Some(newer.cert_sha256_base64));
    server.abort();
}

/// A renewal whose key write fails leaves the expiring certificate on disk,
/// so the next check renews again instead of reading a fresh certificate
/// that no listener serves.
#[tokio::test]
async fn failed_key_write_keeps_the_expiring_certificate_for_a_retry() {
    let dir = tempfile::tempdir().expect("owned TLS directory");
    let cert_pem = dir.path().join("cert.pem");
    let key_pem = dir.path().join("key.pem");
    persist_tls_pem_valid_for(&cert_pem, &key_pem, time::Duration::hours(12))
        .expect("expiring certificate");
    let old = tls_material_from_pem_files(&cert_pem, &key_pem).expect("old TLS");
    let before = std::fs::read(&cert_pem).expect("certificate bytes");
    // The key path cannot be replaced by a file.
    std::fs::remove_file(&key_pem).expect("remove key");
    std::fs::create_dir(&key_pem).expect("key path blocked");
    let endpoint = quinn::Endpoint::server(
        quic_server_config(old.certs.clone(), old.key.clone_key()).expect("QUIC config"),
        "127.0.0.1:0".parse().expect("owned address"),
    )
    .expect("QUIC endpoint");
    let state = Arc::new(AppState::new(Arc::new(chappe::Bus::default())));
    state.set_tls_cert_sha256_base64(old.cert_sha256_base64.clone());
    let renewal = TlsRenewal {
        cert_pem: cert_pem.clone(),
        key_pem: key_pem.clone(),
        https: None,
    };
    let mut served = ServedTls {
        quic: old.cert_sha256_base64.clone(),
        https: None,
    };

    assert!(renew_tls(&state, &endpoint, &renewal, &mut served)
        .await
        .is_err());

    assert_eq!(std::fs::read(&cert_pem).expect("certificate bytes"), before);
    assert!(tls_needs_renewal(
        &cert_pem,
        time::OffsetDateTime::now_utc()
    ));
    assert_eq!(state.tls_cert_sha256_base64(), Some(old.cert_sha256_base64));
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .expect("TLS directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
        .collect();
    assert!(leftovers.is_empty(), "no temp files left behind");
}
