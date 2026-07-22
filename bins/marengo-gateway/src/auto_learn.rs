//! Auto Learn reverse proxy: Consul → gateway → local compound-auto-learn (127.0.0.1:8787).
//!
//! Fail-closed operator auth; upstream URL is hardcoded (no SSRF). Injects Bearer from
//! `AUTO_LEARN_TOKEN`. Concurrency ≤ 1; long total timeout covers 2× Agent.prompt.

use std::sync::OnceLock;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use tokio::sync::Mutex;
use tracing::warn;

/// Hardcoded upstream — never overridden in production.
const UPSTREAM_URL: &str = "http://127.0.0.1:8787/v1/auto-learn";
const MAX_BODY_BYTES: usize = 256 * 1024;
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(600);

const OPERATOR_TOKEN_ENV: &str = "MARENGO_AUTO_LEARN_OPERATOR_TOKEN";
const OPERATOR_TOKEN_HEADER: &str = "x-marengo-auto-learn-token";
const UPSTREAM_TOKEN_ENV: &str = "AUTO_LEARN_TOKEN";

static PROXY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn proxy_lock() -> &'static Mutex<()> {
    PROXY_LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Timeouts {
    connect: Duration,
    total: Duration,
}

fn timeouts() -> Timeouts {
    #[cfg(test)]
    if let Some(t) = test_support::timeouts_override() {
        return t;
    }
    Timeouts {
        connect: DEFAULT_CONNECT_TIMEOUT,
        total: DEFAULT_REQUEST_TIMEOUT,
    }
}

fn upstream_url() -> String {
    #[cfg(test)]
    if let Some(url) = test_support::upstream_url_override() {
        return url;
    }
    UPSTREAM_URL.to_string()
}

#[cfg(test)]
mod test_support {
    use super::Timeouts;
    use std::sync::{Mutex, OnceLock};

    #[derive(Default, Clone)]
    pub struct TestKnobs {
        pub upstream_url: Option<String>,
        pub timeouts: Option<Timeouts>,
    }

    static TEST_KNOBS: OnceLock<Mutex<TestKnobs>> = OnceLock::new();

    fn knobs_lock() -> std::sync::MutexGuard<'static, TestKnobs> {
        TEST_KNOBS
            .get_or_init(|| Mutex::new(TestKnobs::default()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn timeouts_override() -> Option<Timeouts> {
        knobs_lock().timeouts
    }

    pub fn upstream_url_override() -> Option<String> {
        knobs_lock().upstream_url.clone()
    }

    pub fn set_test_knobs(knobs: TestKnobs) {
        *knobs_lock() = knobs;
    }

    pub fn clear_test_knobs() {
        set_test_knobs(TestKnobs::default());
    }
}

fn operator_token_from_env() -> Option<String> {
    std::env::var(OPERATOR_TOKEN_ENV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn upstream_token_from_env() -> Option<String> {
    std::env::var(UPSTREAM_TOKEN_ENV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Fail closed when operator token env is unset/empty or header mismatches.
pub fn authorize_auto_learn(headers: &HeaderMap) -> Result<(), StatusCode> {
    let Some(expected) = operator_token_from_env() else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let provided = headers
        .get(OPERATOR_TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim);
    if provided == Some(expected.as_str()) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

fn json_error(status: StatusCode, error: &'static str) -> Response {
    (status, Json(ErrorBody { error })).into_response()
}

fn build_client(timeouts: Timeouts) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(timeouts.connect)
        .timeout(timeouts.total)
        .build()
}

/// `POST /v1/auto-learn` — authenticated reverse proxy to local Auto Learn.
pub async fn post_auto_learn(headers: HeaderMap, body: Bytes) -> Response {
    if let Err(status) = authorize_auto_learn(&headers) {
        return status.into_response();
    }

    if body.len() > MAX_BODY_BYTES {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }

    let Some(upstream_token) = upstream_token_from_env() else {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "auto_learn_upstream_token_missing",
        );
    };

    let Ok(_guard) = proxy_lock().try_lock() else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };

    let timeouts = timeouts();
    let client = match build_client(timeouts) {
        Ok(c) => c,
        Err(err) => {
            warn!(error = %err, "auto-learn client build failed");
            return json_error(StatusCode::SERVICE_UNAVAILABLE, "auto_learn_unavailable");
        }
    };

    let url = upstream_url();
    let mut request = client
        .post(&url)
        .header(header::AUTHORIZATION, format!("Bearer {upstream_token}"))
        .body(body.to_vec());

    // Forward content-type only; strip any incoming Authorization before inject above.
    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        request = request.header(header::CONTENT_TYPE, ct.clone());
    }

    // Client disconnect cancels this future and drops the in-flight reqwest request.
    let upstream = match request.send().await {
        Ok(resp) => resp,
        Err(err) => {
            warn!(error = %err, "auto-learn upstream request failed");
            return json_error(StatusCode::SERVICE_UNAVAILABLE, "auto_learn_unavailable");
        }
    };

    let status = StatusCode::from_u16(upstream.status().as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = upstream
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned();
    let bytes = match upstream.bytes().await {
        Ok(b) => b,
        Err(err) => {
            warn!(error = %err, "auto-learn upstream body read failed");
            return json_error(StatusCode::SERVICE_UNAVAILABLE, "auto_learn_unavailable");
        }
    };

    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = status;
    if let Some(ct) = content_type {
        response.headers_mut().insert(header::CONTENT_TYPE, ct);
    }
    response
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use axum::body::Body;
    use axum::extract::DefaultBodyLimit;
    use axum::http::Request;
    use axum::routing::post;
    use axum::Router;
    use std::net::SocketAddr;
    use tower::ServiceExt;

    use crate::logs::lock_test_env;

    use super::test_support::{clear_test_knobs, set_test_knobs, TestKnobs};

    const OP_TOKEN: &str = "auto-learn-op-token";
    const UP_TOKEN: &str = "auto-learn-upstream-token";

    fn router() -> Router {
        Router::new().route(
            "/v1/auto-learn",
            post(post_auto_learn).layer(DefaultBodyLimit::max(MAX_BODY_BYTES)),
        )
    }

    async fn read_json_error(res: Response) -> (StatusCode, String) {
        let status = res.status();
        let body = axum::body::to_bytes(res.into_body(), 64 * 1024)
            .await
            .expect("body");
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
        let error = parsed
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        (status, error)
    }

    async fn spawn_mock_upstream(
        expected_bearer: Option<&'static str>,
        delay: Duration,
        status: StatusCode,
        response_body: &'static str,
    ) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let app = Router::new().route(
            "/v1/auto-learn",
            post(move |headers: HeaderMap, _body: Bytes| async move {
                let auth = headers
                    .get(header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string);
                if let Some(expected) = expected_bearer {
                    let want = format!("Bearer {expected}");
                    if auth.as_deref() != Some(want.as_str()) {
                        return (
                            StatusCode::UNAUTHORIZED,
                            Json(serde_json::json!({"error": "bad_bearer"})),
                        )
                            .into_response();
                    }
                }
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                (status, response_body).into_response()
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock");
        let addr = listener.local_addr().expect("addr");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve mock");
        });
        tokio::task::yield_now().await;
        (addr, handle)
    }

    #[tokio::test]
    async fn rejects_missing_operator_token() {
        let _env = lock_test_env();
        clear_test_knobs();
        std::env::remove_var(OPERATOR_TOKEN_ENV);
        std::env::remove_var(UPSTREAM_TOKEN_ENV);

        let res = router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auto-learn")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"goal":"x"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rejects_wrong_operator_token() {
        let _env = lock_test_env();
        clear_test_knobs();
        std::env::set_var(OPERATOR_TOKEN_ENV, OP_TOKEN);
        std::env::set_var(UPSTREAM_TOKEN_ENV, UP_TOKEN);

        let res = router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auto-learn")
                    .header("content-type", "application/json")
                    .header(OPERATOR_TOKEN_HEADER, "wrong")
                    .body(Body::from(r#"{"goal":"x"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        std::env::remove_var(OPERATOR_TOKEN_ENV);
        std::env::remove_var(UPSTREAM_TOKEN_ENV);
    }

    #[tokio::test]
    async fn missing_upstream_token_returns_503() {
        let _env = lock_test_env();
        clear_test_knobs();
        std::env::set_var(OPERATOR_TOKEN_ENV, OP_TOKEN);
        std::env::remove_var(UPSTREAM_TOKEN_ENV);

        let res = router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auto-learn")
                    .header("content-type", "application/json")
                    .header(OPERATOR_TOKEN_HEADER, OP_TOKEN)
                    .body(Body::from(r#"{"goal":"x"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        let (status, error) = read_json_error(res).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error, "auto_learn_upstream_token_missing");

        std::env::remove_var(OPERATOR_TOKEN_ENV);
    }

    #[tokio::test]
    async fn upstream_down_returns_503() {
        let _env = lock_test_env();
        clear_test_knobs();
        std::env::set_var(OPERATOR_TOKEN_ENV, OP_TOKEN);
        std::env::set_var(UPSTREAM_TOKEN_ENV, UP_TOKEN);
        // Dead port — nothing listening.
        set_test_knobs(TestKnobs {
            upstream_url: Some("http://127.0.0.1:1/v1/auto-learn".into()),
            timeouts: Some(Timeouts {
                connect: Duration::from_millis(200),
                total: Duration::from_secs(2),
            }),
        });

        let res = router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auto-learn")
                    .header("content-type", "application/json")
                    .header(OPERATOR_TOKEN_HEADER, OP_TOKEN)
                    .body(Body::from(r#"{"goal":"x"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        let (status, error) = read_json_error(res).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error, "auto_learn_unavailable");

        clear_test_knobs();
        std::env::remove_var(OPERATOR_TOKEN_ENV);
        std::env::remove_var(UPSTREAM_TOKEN_ENV);
    }

    #[tokio::test]
    async fn injects_bearer_and_maps_upstream() {
        let _env = lock_test_env();
        clear_test_knobs();
        std::env::set_var(OPERATOR_TOKEN_ENV, OP_TOKEN);
        std::env::set_var(UPSTREAM_TOKEN_ENV, UP_TOKEN);

        let (addr, handle) = spawn_mock_upstream(
            Some(UP_TOKEN),
            Duration::ZERO,
            StatusCode::OK,
            r#"{"ok":true,"source":"mock"}"#,
        )
        .await;
        set_test_knobs(TestKnobs {
            upstream_url: Some(format!("http://{addr}/v1/auto-learn")),
            timeouts: Some(Timeouts {
                connect: Duration::from_secs(2),
                total: Duration::from_secs(10),
            }),
        });

        let res = router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auto-learn")
                    .header("content-type", "application/json")
                    .header(OPERATOR_TOKEN_HEADER, OP_TOKEN)
                    // Must be stripped / ignored — upstream only sees injected Bearer.
                    .header(header::AUTHORIZATION, "Bearer client-should-not-leak")
                    .body(Body::from(r#"{"goal":"proxy-me"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");

        let status = res.status();
        let body = axum::body::to_bytes(res.into_body(), 64 * 1024)
            .await
            .expect("body");
        assert_eq!(status, StatusCode::OK, "body={}", String::from_utf8_lossy(&body));
        let parsed: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(parsed["ok"], true);
        assert_eq!(parsed["source"], "mock");

        handle.abort();
        clear_test_knobs();
        std::env::remove_var(OPERATOR_TOKEN_ENV);
        std::env::remove_var(UPSTREAM_TOKEN_ENV);
    }

    #[tokio::test]
    async fn slow_upstream_completes_within_timeout() {
        let _env = lock_test_env();
        clear_test_knobs();
        std::env::set_var(OPERATOR_TOKEN_ENV, OP_TOKEN);
        std::env::set_var(UPSTREAM_TOKEN_ENV, UP_TOKEN);

        // Short knobs: delay > "typical short" wait, still under total timeout.
        let delay = Duration::from_secs(2);
        let (addr, handle) = spawn_mock_upstream(
            Some(UP_TOKEN),
            delay,
            StatusCode::OK,
            r#"{"ok":true,"slow":true}"#,
        )
        .await;
        set_test_knobs(TestKnobs {
            upstream_url: Some(format!("http://{addr}/v1/auto-learn")),
            timeouts: Some(Timeouts {
                connect: Duration::from_secs(2),
                total: Duration::from_secs(10),
            }),
        });

        let started = std::time::Instant::now();
        let res = router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auto-learn")
                    .header("content-type", "application/json")
                    .header(OPERATOR_TOKEN_HEADER, OP_TOKEN)
                    .body(Body::from(r#"{"goal":"slow"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        let elapsed = started.elapsed();

        let status = res.status();
        let body = axum::body::to_bytes(res.into_body(), 64 * 1024)
            .await
            .expect("body");
        assert_eq!(status, StatusCode::OK, "body={}", String::from_utf8_lossy(&body));
        assert!(
            elapsed >= delay,
            "expected to wait for slow upstream, elapsed={elapsed:?}"
        );
        assert!(elapsed < Duration::from_secs(10));

        handle.abort();
        clear_test_knobs();
        std::env::remove_var(OPERATOR_TOKEN_ENV);
        std::env::remove_var(UPSTREAM_TOKEN_ENV);
    }
}
