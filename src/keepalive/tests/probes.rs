use super::super::*;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::IntoResponse,
    routing::any,
    Router,
};
use std::sync::{Arc, Mutex};
use tokio::{net::TcpListener, task::JoinHandle};

#[derive(Clone)]
struct ProbeState {
    status: StatusCode,
    body: String,
    requests: Arc<Mutex<Vec<(String, Option<String>)>>>,
}

struct ProbeServer {
    base_url: String,
    state: ProbeState,
    task: JoinHandle<()>,
}

impl Drop for ProbeServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl ProbeServer {
    async fn start(status: StatusCode, body: &str) -> Self {
        async fn respond(
            State(state): State<ProbeState>,
            uri: Uri,
            headers: HeaderMap,
        ) -> impl IntoResponse {
            state.requests.lock().unwrap().push((
                uri.path().to_string(),
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string),
            ));
            (state.status, [("location", "/signin")], state.body)
        }

        let state = ProbeState {
            status,
            body: body.to_string(),
            requests: Arc::new(Mutex::new(Vec::new())),
        };
        let app = Router::new()
            .fallback(any(respond))
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base_url,
            state,
            task,
        }
    }

    fn request(&self, adapter: &str) -> GatewayKeepaliveEnsureRequest {
        GatewayKeepaliveEnsureRequest {
            project_id: None,
            session_key: None,
            previous_response_id: None,
            credential_id: None,
            account_name: None,
            provider_account_id: "probe-provider".to_string(),
            adapter: adapter.to_string(),
            base_url: self.base_url.clone(),
            model: "qwen-lora".to_string(),
            api_key: Some("probe-token".to_string()),
            headers: HashMap::new(),
            extra_body: Some(HashMap::from([("realmId".to_string(), json!("realm-1"))])),
            session_auth: Some(SessionAuthConfig {
                transport: "bearer".to_string(),
                primary_cookie_name: None,
                secondary_cookie_name: None,
                header_name: Some("authorization".to_string()),
                expires_at: None,
            }),
            expires_at: None,
            runtime_state_object_key: Some("runtime/probe-state.json".to_string()),
        }
    }
}

async fn ensure(
    input: GatewayKeepaliveEnsureRequest,
) -> Result<GatewayKeepaliveEnsureResponse, GatewayError> {
    let redis = deadpool_redis::Config::from_url("redis://127.0.0.1:1")
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();
    // No credential/project IDs: these cases exercise probes without external stores.
    ensure_credential_runtime(&redis, None, &Client::new(), input).await
}

#[tokio::test]
async fn admission_probes_preserve_early_failure_and_successful_fallthrough() {
    let cases = [
        (
            "chataibot_compatible",
            StatusCode::OK,
            r#"{"leftAnswersCount":10}"#,
            true,
            "Credential runtime material is ready.",
            "/api/user/answers-count/v2",
        ),
        (
            "chataibot_compatible",
            StatusCode::OK,
            r#"{"leftAnswersCount":0}"#,
            false,
            "below the required minimum",
            "/api/user/answers-count/v2",
        ),
        (
            "chataibot_compatible",
            StatusCode::SERVICE_UNAVAILABLE,
            "quota offline",
            false,
            "quota offline",
            "/api/user/answers-count/v2",
        ),
        (
            "lumalabs_compatible",
            StatusCode::OK,
            "board content",
            true,
            "Credential runtime material is ready.",
            "/board/realm-1",
        ),
        (
            "lumalabs_compatible",
            StatusCode::FOUND,
            "",
            false,
            "redirected with HTTP",
            "/board/realm-1",
        ),
        (
            "lumalabs_compatible",
            StatusCode::OK,
            "Please SIGN IN",
            false,
            "auth/challenge page",
            "/board/realm-1",
        ),
        (
            "producer_compatible",
            StatusCode::OK,
            "{}",
            true,
            "Credential runtime material is ready.",
            "/__api/billing/credits",
        ),
        (
            "producer_compatible",
            StatusCode::FORBIDDEN,
            "credits denied",
            false,
            "credits denied",
            "/__api/billing/credits",
        ),
    ];

    for (adapter, status, body, ready, message, path) in cases {
        let server = ProbeServer::start(status, body).await;
        let response = ensure(server.request(adapter)).await.unwrap();
        assert_eq!(response.ready, ready, "{adapter}: {body}");
        assert!(response.message.as_deref().unwrap().contains(message));
        assert_eq!(response.api_key.as_deref(), Some("probe-token"));
        assert_eq!(
            response.runtime_state_object_key.as_deref(),
            Some("runtime/probe-state.json")
        );
        assert_eq!(response.session_auth.unwrap().transport, "bearer");
        assert_eq!(
            *server.state.requests.lock().unwrap(),
            vec![(path.to_string(), Some("Bearer probe-token".to_string()))],
            "{adapter} must issue exactly one probe and retain auth"
        );
    }
}

#[tokio::test]
async fn expired_probe_material_returns_before_network_io() {
    for adapter in ["chataibot_compatible", "producer_compatible"] {
        let server = ProbeServer::start(StatusCode::OK, "{}").await;
        let mut input = server.request(adapter);
        input.expires_at = Some("2000-01-01T00:00:00Z".to_string());
        let response = ensure(input).await.unwrap();
        assert!(!response.ready);
        assert!(response.message.unwrap().contains("expired"));
        assert_eq!(response.expires_at.as_deref(), Some("2000-01-01T00:00:00Z"));
        assert!(server.state.requests.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn missing_luma_realm_returns_before_network_io() {
    let server = ProbeServer::start(StatusCode::OK, "board content").await;
    let mut input = server.request("lumalabs_compatible");
    input.extra_body = None;
    let response = ensure(input).await.unwrap();
    assert!(!response.ready);
    assert!(response.message.unwrap().contains("extraBody.realmId"));
    assert!(server.state.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn missing_chataibot_quota_propagates_error_instead_of_fallthrough() {
    let server = ProbeServer::start(StatusCode::OK, "{}").await;
    let error = ensure(server.request("chataibot_compatible"))
        .await
        .unwrap_err();
    assert!(format!("{error:?}").contains("leftAnswersCount"));
    assert_eq!(server.state.requests.lock().unwrap().len(), 1);
}
