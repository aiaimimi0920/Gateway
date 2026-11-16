//! Synthetic loopback only, including ambient-header leaks, pacing, redirects and error ambiguity.
use super::*;
use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    Router,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Reply {
    status: u16,
    body: String,
    headers: Vec<(&'static str, String)>,
}
impl Reply {
    fn json(status: u16, value: Value) -> Self {
        Self {
            status,
            body: value.to_string(),
            headers: vec![],
        }
    }
}
#[derive(Clone)]
struct Captured {
    path: String,
    headers: HeaderMap,
    form: String,
}
type Shared = Arc<Mutex<(std::collections::VecDeque<Reply>, Vec<Captured>)>>;
struct Server {
    origin: String,
    state: Shared,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    async fn new(replies: Vec<Reply>) -> Self {
        let state: Shared = Arc::new(Mutex::new((replies.into(), Vec::new())));
        let app = Router::new()
            .fallback(handle)
            .with_state(Arc::clone(&state));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            origin,
            state,
            task,
        }
    }
    fn requests(&self) -> Vec<Captured> {
        self.state.lock().unwrap().1.clone()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn handle(State(state): State<Shared>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, 65_536).await.unwrap();
    let captured = Captured {
        path: parts.uri.path().into(),
        headers: parts.headers,
        form: String::from_utf8(bytes.to_vec()).unwrap(),
    };
    let mut state = state.lock().unwrap();
    state.1.push(captured);
    let reply = state.0.pop_front().expect("no unexpected extra request");
    let mut response = Response::builder().status(StatusCode::from_u16(reply.status).unwrap());
    for (key, value) in reply.headers {
        response = response.header(key, value);
    }
    response.body(Body::from(reply.body)).unwrap()
}
fn http() -> rquest::Client {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert("authorization", "Bearer ambient-secret".parse().unwrap());
    headers.insert("x-ambient-secret", "ambient-secret".parse().unwrap());
    rquest::Client::builder()
        .no_proxy()
        .default_headers(headers)
        .build()
        .unwrap()
}
fn challenge(interval: Option<u64>, ttl: u64) -> Value {
    let mut value = json!({"device_code":"private-device-code","user_code":"ABCD-EFGH",
        "verification_uri":"https://portal.nousresearch.com/device", "expires_in":ttl});
    if let Some(interval) = interval {
        value["interval"] = json!(interval);
    }
    value
}

#[tokio::test]
async fn nous_refresh_sends_rt_only_in_header_and_returns_receipt_before_jwt_validation() {
    let server = Server::new(vec![Reply::json(
        200,
        json!({"access_token":"unusable-jwt",
        "refresh_token":"rotated-rt","expires_in":"bad"}),
    )])
    .await;
    transport::TEST_ORIGIN
        .scope(server.origin.clone(), async {
            let receipt = client()
                .prepare_refresh("single-use-rt")
                .unwrap()
                .execute(&http())
                .await
                .unwrap();
            assert_eq!(receipt.refresh_token().unwrap(), "rotated-rt");
            assert!(receipt.admit(&client(), None).is_err());
        })
        .await;
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.path, "/api/oauth/token");
    assert_eq!(request.headers["x-nous-refresh-token"], "single-use-rt");
    assert!(request.headers.get("authorization").is_none());
    assert!(request.headers.get("x-ambient-secret").is_none());
    let form: std::collections::HashMap<_, _> =
        url::form_urlencoded::parse(request.form.as_bytes())
            .into_owned()
            .collect();
    assert_eq!(form.len(), 2);
    assert_eq!(form["client_id"], client().client_id());
    assert_eq!(form["grant_type"], "refresh_token");
    assert!(!request.form.contains("single-use-rt"));
}

#[tokio::test]
async fn nous_refresh_classifies_dead_waf_temporary_and_unknown_without_secret_errors() {
    let cases = [
        (
            Reply::json(
                400,
                json!({"error":"invalid_grant","error_description":"private-prose"}),
            ),
            OAuthError::ReauthorizationRequired,
        ),
        (
            Reply::json(401, json!({"error":"invalid_token"})),
            OAuthError::ReauthorizationRequired,
        ),
        (
            Reply::json(403, json!({"error":"refresh_token_reused"})),
            OAuthError::ReauthorizationRequired,
        ),
        (
            Reply {
                status: 403,
                body: "private-html".into(),
                headers: vec![],
            },
            OAuthError::Rejected,
        ),
        (
            Reply {
                status: 401,
                body: String::new(),
                headers: vec![],
            },
            OAuthError::Rejected,
        ),
        (
            Reply {
                status: 403,
                body: "private-waf".into(),
                headers: vec![("x-vercel-mitigated", "deny".into())],
            },
            OAuthError::UpstreamBlocked,
        ),
        (
            Reply::json(429, json!({"error":"invalid_grant"})),
            OAuthError::TemporarilyUnavailable,
        ),
        (
            Reply::json(503, json!({"error":"invalid_grant"})),
            OAuthError::TemporarilyUnavailable,
        ),
        (
            Reply::json(408, json!({})),
            OAuthError::TemporarilyUnavailable,
        ),
    ];
    for (reply, expected) in cases {
        let server = Server::new(vec![reply]).await;
        let error = transport::TEST_ORIGIN
            .scope(
                server.origin.clone(),
                client()
                    .prepare_refresh("one-attempt-rt")
                    .unwrap()
                    .execute(&http()),
            )
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error}").contains("private-"));
        assert_eq!(server.requests().len(), 1);
    }
}

#[tokio::test]
async fn nous_redirect_and_oversized_or_ambiguous_success_never_trigger_a_second_request() {
    let destination = Server::new(vec![]).await;
    for reply in [
        Reply {
            status: 307,
            body: String::new(),
            headers: vec![("location", format!("{}/stolen", destination.origin))],
        },
        Reply {
            status: 200,
            body: "x".repeat(65_537),
            headers: vec![],
        },
        Reply {
            status: 200,
            body: r#"{"refresh_token":"a","refresh_token":"b"}"#.into(),
            headers: vec![],
        },
    ] {
        let server = Server::new(vec![reply]).await;
        assert!(transport::TEST_ORIGIN
            .scope(
                server.origin.clone(),
                client()
                    .prepare_refresh("single-attempt")
                    .unwrap()
                    .execute(&http())
            )
            .await
            .is_err());
        assert_eq!(server.requests().len(), 1);
    }
    assert!(destination.requests().is_empty());
}

#[tokio::test]
async fn nous_device_default_interval_is_five_seconds_and_cancellation_stops_pacing() {
    let server = Server::new(vec![Reply::json(200, challenge(None, 300))]).await;
    transport::TEST_ORIGIN
        .scope(server.origin.clone(), async {
            let http = http();
            let client = client();
            let mut device = client.begin_device(&http).await.unwrap();
            assert_eq!(device.interval(), Duration::from_secs(5));
            assert!(device.poll_after() > Duration::from_secs(4));
            assert!(tokio::time::timeout(
                Duration::from_millis(50),
                client.poll_device(&http, &mut device)
            )
            .await
            .is_err());
            assert_eq!(server.requests().len(), 1);
            assert!(!format!("{device:?}").contains("private-device"));
        })
        .await;
}

#[tokio::test]
async fn nous_device_pending_and_slow_down_preserve_full_server_pacing() {
    let server = Server::new(vec![
        Reply::json(200, challenge(Some(1), 300)),
        Reply::json(400, json!({"error":"authorization_pending"})),
        Reply::json(400, json!({"error":"slow_down"})),
    ])
    .await;
    transport::TEST_ORIGIN
        .scope(server.origin.clone(), async {
            let http = http();
            let client = client();
            let mut device = client.begin_device(&http).await.unwrap();
            let start = tokio::time::Instant::now();
            assert!(matches!(
                client.poll_device(&http, &mut device).await.unwrap(),
                DevicePoll::Pending
            ));
            assert!(start.elapsed() >= Duration::from_secs(1));
            assert!(matches!(
                client.poll_device(&http, &mut device).await.unwrap(),
                DevicePoll::SlowDown
            ));
            assert_eq!(device.interval(), Duration::from_secs(6));
            assert!(device.poll_after() > Duration::from_secs(5));
            assert!(tokio::time::timeout(
                Duration::from_millis(50),
                client.poll_device(&http, &mut device)
            )
            .await
            .is_err());
            let requests = server.requests();
            assert_eq!(requests.len(), 3);
            assert!(requests[0].form.contains("scope=inference%3Ainvoke"));
            for request in &requests[1..] {
                assert_eq!(request.path, "/api/oauth/token");
                assert!(request.form.contains("device_code=private-device-code"));
                assert!(request.headers.get("x-nous-refresh-token").is_none());
            }
        })
        .await;
}

#[tokio::test]
async fn nous_device_never_sends_a_poll_after_the_shorter_server_deadline() {
    let server = Server::new(vec![Reply::json(200, challenge(None, 1))]).await;
    transport::TEST_ORIGIN
        .scope(server.origin.clone(), async {
            let http = http();
            let client = client();
            let mut device = client.begin_device(&http).await.unwrap();
            assert_eq!(
                client.poll_device(&http, &mut device).await.unwrap_err(),
                OAuthError::Expired
            );
            assert_eq!(server.requests().len(), 1);
        })
        .await;
}

#[tokio::test]
async fn nous_device_success_delivers_a_receipt_once_and_cannot_be_polled_again() {
    let server = Server::new(vec![
        Reply::json(200, challenge(Some(1), 300)),
        Reply::json(200, body(Some("subject"))),
    ])
    .await;
    transport::TEST_ORIGIN
        .scope(server.origin.clone(), async {
            let http = http();
            let client = client();
            let mut device = client.begin_device(&http).await.unwrap();
            assert_eq!(
                OAuthClient::new("different-client")
                    .unwrap()
                    .poll_device(&http, &mut device)
                    .await
                    .unwrap_err(),
                OAuthError::InvalidInput
            );
            let DevicePoll::Received {
                tokens,
                challenge_expired,
            } = client.poll_device(&http, &mut device).await.unwrap()
            else {
                panic!("missing device receipt")
            };
            assert!(!challenge_expired);
            tokens.admit(&client, None).unwrap();
            assert_eq!(
                client.poll_device(&http, &mut device).await.unwrap_err(),
                OAuthError::Expired
            );
            assert_eq!(server.requests().len(), 2);
        })
        .await;
}

#[tokio::test]
async fn nous_models_are_a_read_only_bearer_call_to_the_accepted_fixed_target() {
    let tokens = receipt(body(Some("subject")))
        .admit(&client(), None)
        .unwrap();
    let server = Server::new(vec![
        Reply::json(200, json!({"data":[{"id":"org/model"}]})),
        Reply::json(200, json!({"data":[]})),
        Reply::json(401, json!({"error":"unauthorized"})),
    ])
    .await;
    transport::TEST_ORIGIN
        .scope(server.origin.clone(), async {
            let http = http();
            assert_eq!(
                tokens
                    .context
                    .model_catalog(&http, &tokens.access_token)
                    .await
                    .unwrap()
                    .models[0]
                    .id,
                "org/model"
            );
            assert!(tokens
                .context
                .model_catalog(&http, &tokens.access_token)
                .await
                .unwrap()
                .models
                .is_empty());
            assert!(tokens
                .context
                .model_catalog(&http, &tokens.access_token)
                .await
                .is_err());
        })
        .await;
    let requests = server.requests();
    assert_eq!(requests.len(), 3);
    for request in requests {
        assert_eq!(request.path, "/v1/models");
        assert_eq!(
            request.headers["authorization"],
            format!("Bearer {}", tokens.access_token)
        );
        assert!(request.form.is_empty());
        assert!(request.headers.get("x-nous-refresh-token").is_none());
        assert!(request.headers.get("x-ambient-secret").is_none());
    }
}
