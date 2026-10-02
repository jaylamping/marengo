//! Actual loopback QUIC admission and framing; certificate pinned to the fixture.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;
use crate::access::{AccessPolicy, Capability};
use crate::state::AppState;
use armee_proto::{Envelope, Heartbeat, LogEvent};

const TOKEN: &str = "isolated-quic-read-fixture";

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
        let client = web_transport_quinn::ClientBuilder::new()
            .with_server_certificates(tls.certs.clone())
            .expect("pin owned certificate");
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
