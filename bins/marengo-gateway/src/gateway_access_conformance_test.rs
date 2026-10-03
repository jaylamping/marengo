//! Candidate conformance through actual routes/listeners and an isolated Bus.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use armee_proto::prost::Message;
use armee_proto::{EnableRequest, Envelope};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chappe::Bus;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;

use crate::access::{AccessPolicy, Capability};
use crate::state::AppState;

const TOKEN: &str = "isolated-access-conformance-operator";
const CHILD: &str = "MARENGO_ACCESS_CONFORMANCE_CHILD";

async fn isolated_child(test_name: &'static str) -> bool {
    if std::env::var(CHILD).as_deref() == Ok(test_name) {
        return false;
    }
    let output = tokio::task::spawn_blocking(move || {
        let mut child =
            std::process::Command::new(std::env::current_exe().expect("test executable"));
        child
            .arg(test_name)
            .args(["--exact", "--nocapture"])
            .env(CHILD, test_name);
        for key in [
            "LOG",
            "OPERATOR",
            "CONTROL",
            "CALIBRATION",
            "CONFIG",
            "MANAGEMENT",
            "READ",
        ] {
            child.env_remove(format!("MARENGO_GATEWAY_{key}_TOKEN"));
        }
        child.env_remove("MARENGO_GATEWAY_ALLOWED_ORIGINS");
        child.output().expect("fixture child")
    })
    .await
    .expect("child worker");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

#[tokio::test]
async fn all_protected_routes_refuse_before_body_files_publication_or_subscription() {
    const NAME: &str = "gateway_access_conformance_test::all_protected_routes_refuse_before_body_files_publication_or_subscription";
    if isolated_child(NAME).await {
        return;
    }
    let fixture = tempfile::tempdir().expect("owned fixture root");
    std::env::set_var("MARENGO_ROOT", fixture.path());
    std::env::set_var("MARENGO_CONFIG_DIR", fixture.path().join("config"));
    std::env::set_var("MARENGO_GATEWAY_OPERATOR_TOKEN", TOKEN);
    let bus = Arc::new(Bus::default());
    let state = Arc::new(AppState::new(Arc::clone(&bus)));
    // Capture once: changing process configuration cannot replace a running policy.
    std::env::set_var("MARENGO_GATEWAY_OPERATOR_TOKEN", "different-after-startup");
    let app = crate::http::router(Arc::clone(&state), None);
    let mut receivers: Vec<_> = [
        "robot/enable",
        "robot/testing/mit_command_batch",
        "robot/set_zero",
        "robot/actuator/command",
        "robot/motor_status_poll",
        "robot/active_reporting_lease",
    ]
    .iter()
    .map(|topic| bus.subscribe(topic))
    .collect();
    let routes = [
        ("GET", "/snapshot/logs/recent", 503),
        ("GET", "/logs/sessions", 503),
        ("GET", "/logs/sessions/latest/candump", 503),
        ("GET", "/logs/sessions/latest/candump/summary", 503),
        ("GET", "/logs/sessions/fixture/bench", 503),
        ("GET", "/logs/sessions/fixture/trace", 503),
        ("GET", "/logs/sessions/fixture/candump", 503),
        ("GET", "/logs/sessions/fixture/candump/summary", 503),
        ("GET", "/logs/sessions/fixture/download?type=bench", 503),
        ("GET", "/logs/structured", 503),
        ("GET", "/config/snapshot", 503),
        ("GET", "/hardware/completeness", 500),
        ("GET", "/hardware/commissioning-scope", 200),
        ("GET", "/settings", 503),
        ("GET", "/hardware/urdf/archive", 200),
        ("POST", "/config/patch", 400),
        ("POST", "/hardware/urdf/upload", 400),
        ("POST", "/hardware/urdf/resolve-preview", 400),
        ("POST", "/hardware/urdf/activate", 400),
        ("POST", "/hardware/urdf/archive/upload-fixture/restore", 404),
        ("PUT", "/hardware/commissioning-scope", 400),
        ("DELETE", "/hardware/commissioning-scope", 200),
        ("POST", "/control/restart-marengo-pi", 400),
        ("POST", "/control/deploy", 400),
        ("POST", "/command/enable", 400),
        ("POST", "/command/testing_mit", 400),
        ("POST", "/command/set_zero", 400),
        ("POST", "/command/active_reporting_lease", 400),
        ("POST", "/command/motor_status_poll", 400),
        ("POST", "/command/actuator", 400),
    ];
    for (method, uri, valid_status) in routes {
        for (credential, origin, expected) in [
            (None, None, 401),
            (Some("wrong-fixture"), None, 401),
            (Some(TOKEN), Some("https://foreign.fixture"), 403),
            (Some(TOKEN), Some("http://localhost:5173"), valid_status),
        ] {
            let mut request = Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json");
            if let Some(value) = credential {
                request = request.header("authorization", format!("Bearer {value}"));
            }
            if let Some(value) = origin {
                request = request.header("origin", value);
            }
            // Missing/malformed bodies must not be inspected before access refusal.
            let response = app
                .clone()
                .oneshot(
                    request
                        .body(
                            if matches!(uri, "/command/enable" | "/command/testing_mit") {
                                Body::from(vec![0xff])
                            } else {
                                Body::empty()
                            },
                        )
                        .expect("request"),
                )
                .await
                .expect("response");
            assert_eq!(
                response.status().as_u16(),
                expected,
                "{method} {uri}, credential present={}, origin={origin:?}",
                credential.is_some()
            );
            assert_eq!(
                state.envelope_receiver_count(),
                0,
                "no stream subscription: {uri}"
            );
            assert!(
                receivers.iter_mut().all(|rx| rx.try_recv().is_err()),
                "no command publication: {uri}"
            );
            assert_eq!(
                std::fs::read_dir(fixture.path())
                    .expect("owned root")
                    .count(),
                0,
                "no unexpected file work: {uri}"
            );
        }
    }
    for topics in [
        "logs/structured",
        "robot/heartbeat,robot/audit/action",
        "robot/audit/tuning",
        "robot/testing/mit_command_batch",
    ] {
        for token in [None, Some("wrong-fixture")] {
            let mut request = Request::builder().uri(format!("/stream/chappe?topics={topics}"));
            if let Some(token) = token {
                request = request.header("authorization", format!("Bearer {token}"));
            }
            let response = app
                .clone()
                .oneshot(request.body(Body::empty()).expect("stream request"))
                .await
                .expect("stream response");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(state.envelope_receiver_count(), 0);
        }
    }
}

#[tokio::test]
async fn role_matrix_and_body_limits_preserve_independent_admission() {
    use Capability::*;
    let roles = [
        (Control, "/command/enable"),
        (Calibration, "/command/set_zero"),
        (Configuration, "/config/snapshot"),
        (Management, "/control/deploy"),
        (SensitiveRead, "/logs/structured"),
    ];
    for (role, own_path) in roles {
        let policy = AccessPolicy::role_fixture(TOKEN, role).expect("real scoped grant");
        let state = Arc::new(AppState::new(Arc::new(Bus::default())).with_access(policy));
        let app = crate::http::router(state, None);
        for (_, path) in roles {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(
                            if path.starts_with("/logs/") || path == "/config/snapshot" {
                                "GET"
                            } else {
                                "POST"
                            },
                        )
                        .uri(path)
                        .header("authorization", format!("Bearer {TOKEN}"))
                        .header("content-type", "application/json")
                        .body(Body::from(vec![0xff]))
                        .expect("request"),
                )
                .await
                .expect("response");
            let expected = if path != own_path {
                403
            } else if path == "/config/snapshot" || role as u8 == SensitiveRead as u8 {
                503
            } else {
                400
            };
            assert_eq!(
                response.status().as_u16(),
                expected,
                "role={} path={path}",
                role as u8
            );
        }
    }
    for role in [Control, Configuration] {
        let policy = AccessPolicy::role_fixture(TOKEN, role).expect("scoped grant");
        let app = crate::http::router(
            Arc::new(AppState::new(Arc::new(Bus::default())).with_access(policy)),
            None,
        );
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/config/patch")
                    .header("authorization", format!("Bearer {TOKEN}"))
                    .header("content-type", "application/json")
                    .body(Body::from(vec![0xff]))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let state = Arc::new(
        AppState::new(Arc::new(Bus::default()))
            .with_access(AccessPolicy::operator_fixture(TOKEN).expect("operator grant")),
    );
    let app = crate::http::router(state, None);
    for (token, expected) in [(None, 401), (Some(TOKEN), 413)] {
        let mut request = Request::builder().method("POST").uri("/command/enable");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = app
            .clone()
            .oneshot(
                request
                    .body(Body::from(vec![0; 2 * 1024 * 1024 + 1]))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status().as_u16(), expected);
    }
}

#[tokio::test]
async fn actual_http_and_https_listeners_share_access_and_public_controls() {
    let cert =
        rcgen::generate_simple_self_signed(vec!["localhost".into()]).expect("owned TLS cert");
    let der = cert.cert.der().clone();
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(cert.key_pair.serialize_der().into());
    let tls_config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("TLS versions")
    .with_no_client_auth()
    .with_single_cert(vec![der.clone()], key)
    .expect("TLS fixture config");
    let tls_listener = std::net::TcpListener::bind("127.0.0.1:0").expect("owned TLS listener");
    tls_listener
        .set_nonblocking(true)
        .expect("nonblocking owned listener");
    let tls_addr = tls_listener.local_addr().expect("TLS address");
    let bus = Arc::new(Bus::default());
    let state = Arc::new(
        AppState::new(Arc::clone(&bus)).with_access(
            AccessPolicy::operator_fixture(TOKEN)
                .expect("grant")
                .with_robot_https_port(Some(tls_addr.port())),
        ),
    );
    let mut commands = bus.subscribe("robot/enable");
    let app = crate::http::router(Arc::clone(&state), None);
    let tls_handle = axum_server::Handle::new();
    let tls_shutdown = tls_handle.clone();
    let tls_task = tokio::spawn(
        axum_server::from_tcp_rustls(
            tls_listener,
            axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(tls_config)),
        )
        .expect("TLS server")
        .handle(tls_handle)
        .serve(app.clone().into_make_service()),
    );
    let plain_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("owned HTTP listener");
    let plain_addr = plain_listener.local_addr().expect("HTTP address");
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
    let plain_task = tokio::spawn(async move {
        axum::serve(plain_listener, app)
            .with_graceful_shutdown(async {
                let _ = stop_rx.await;
            })
            .await
    });
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(der)
        .expect("trust only owned fixture certificate");
    let client_config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("TLS versions")
    .with_root_certificates(roots)
    .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config));
    for tls in [false, true] {
        for (token, origin, expected) in [
            (None, None, 401),
            (Some("wrong-fixture"), None, 401),
            (Some(TOKEN), Some("https://foreign.fixture"), 403),
            (Some(TOKEN), Some("http://localhost:5173"), 200),
        ] {
            let request = EnableRequest {
                enable: false,
                ..Default::default()
            }
            .encode_to_vec();
            let mut headers=format!("POST /command/enable HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/x-protobuf\r\nContent-Length: {}\r\nConnection: close\r\n",request.len());
            if let Some(token) = token {
                headers.push_str(&format!("Authorization: Bearer {token}\r\n"));
            }
            if let Some(origin) = origin {
                headers.push_str(&format!("Origin: {origin}\r\n"));
            }
            headers.push_str("\r\n");
            let mut bytes = headers.into_bytes();
            bytes.extend(request);
            let response = socket_request(
                tls,
                if tls { tls_addr } else { plain_addr },
                &connector,
                &bytes,
            )
            .await;
            assert!(
                response.starts_with(&format!("HTTP/1.1 {expected}")),
                "tls={tls} response={response}"
            );
            if expected == 200 {
                let env = Envelope::decode(
                    commands
                        .try_recv()
                        .expect("owned bus publication")
                        .as_slice(),
                )
                .expect("envelope");
                assert!(
                    !EnableRequest::decode(env.payload.as_slice())
                        .expect("disable control")
                        .enable
                );
            } else {
                assert!(commands.try_recv().is_err());
            }
        }
        let response = socket_request(
            tls,
            if tls { tls_addr } else { plain_addr },
            &connector,
            b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await;
        assert!(response.starts_with("HTTP/1.1 200"));
    }
    let _ = stop_tx.send(());
    tls_shutdown.shutdown();
    tokio::time::timeout(Duration::from_secs(3), plain_task)
        .await
        .expect("HTTP stopped")
        .expect("HTTP task")
        .expect("HTTP result");
    tokio::time::timeout(Duration::from_secs(3), tls_task)
        .await
        .expect("TLS stopped")
        .expect("TLS task")
        .expect("TLS result");
}

async fn socket_request(
    tls: bool,
    addr: std::net::SocketAddr,
    connector: &tokio_rustls::TlsConnector,
    request: &[u8],
) -> String {
    tokio::time::timeout(Duration::from_secs(3), async {
        let socket = tokio::net::TcpStream::connect(addr)
            .await
            .expect("owned socket");
        let mut out = Vec::new();
        if tls {
            let name = rustls::pki_types::ServerName::try_from("localhost").expect("TLS name");
            let mut socket = connector
                .connect(name, socket)
                .await
                .expect("verified fixture TLS");
            socket.write_all(request).await.expect("TLS request");
            socket.read_to_end(&mut out).await.expect("TLS response");
        } else {
            let mut socket = socket;
            socket.write_all(request).await.expect("HTTP request");
            socket.read_to_end(&mut out).await.expect("HTTP response");
        }
        String::from_utf8(out).expect("HTTP UTF8")
    })
    .await
    .expect("bounded fixture request")
}
