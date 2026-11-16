//! Compile-time task-local token fixture; production HTTP input cannot choose this endpoint.
use super::*;
use axum::{routing::post, Router};
use std::sync::atomic::{AtomicUsize, Ordering};
pub(super) const ROOT: &str = "/v1/internal/gateway/console/accio-auth-sessions";
pub(super) const EXPIRY: &str = "2099-01-01T00:00:00.123456789Z";
pub(super) struct Issuer {
    pub endpoint: String,
    pub hits: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Issuer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Issuer {
    pub async fn new(sg: bool, expiry: &str, pause: Option<Arc<tokio::sync::Notify>>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/token", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let seen = hits.clone();
        let mut data = json!({"accessToken":"synthetic-new-access","refreshToken":"synthetic-new-refresh","expiresAt":expiry});
        if sg {
            data["sgcookie"] = json!("synthetic-new-sg");
        }
        let app = Router::new().route(
            "/token",
            post(move |headers: HeaderMap, Json(body): Json<Value>| {
                let seen = seen.clone();
                let data = data.clone();
                let pause = pause.clone();
                async move {
                    assert!(headers.get("authorization").is_none());
                    assert!(headers.get("cookie").is_none());
                    assert_eq!(body.as_object().unwrap().len(), 4);
                    assert_eq!(body["clientId"], "synthetic-explicit-client");
                    assert_eq!(body["redirectUri"], "http://127.0.0.1:34567/auth/callback");
                    assert_eq!(body["code"], "synthetic-code");
                    assert!(!body["codeVerifier"].as_str().unwrap().is_empty());
                    seen.fetch_add(1, Ordering::SeqCst);
                    if let Some(pause) = pause {
                        pause.notified().await;
                    }
                    Json(json!({"success":true,"data":data}))
                }
            }),
        );
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            endpoint,
            hits,
            task,
        }
    }
}
pub(super) fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"saved-pool","preset":"accio","base_url":"https://phoenix-gw.alibaba.com","explicit_credentials_only":true,"credentials":[]},
        {"id":"unrelated","base_url":"https://example.invalid/v1","api_key":"synthetic-other-secret"}]})).unwrap()
}
pub(super) async fn create(
    state: &Arc<AppState>,
    grant: &str,
    credential: Option<&str>,
    device: Option<&str>,
) -> Value {
    let mut body = json!({"providerId":"saved-pool","expectedRevision":state.route_config.snapshot().revision().id(),
        "clientId":"synthetic-explicit-client","loginSite":"china","callbackPort":34567});
    if let Some(id) = credential {
        body["credentialId"] = json!(id);
    }
    if let Some(device) = device {
        body["utdid"] = json!(device);
    }
    let response =
        support::send_console_json(state, Method::POST, ROOT, body, Some(grant), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    support::parse_json(response).await["session"].clone()
}
pub(super) fn session(
    state: &Arc<AppState>,
    view: &Value,
) -> Arc<crate::console::accio_oauth::Session> {
    let (owner, _) = authorize(
        state,
        &HeaderMap::new(),
        Some(support::MANAGEMENT_TOKEN),
        None,
        false,
    )
    .unwrap();
    state
        .lifecycle
        .accio_oauth
        .get_owned(view["id"].as_str().unwrap(), &owner)
        .unwrap()
}
pub(super) fn callback(view: &Value) -> String {
    let url = url::Url::parse(view["authorizationUrl"].as_str().unwrap()).unwrap();
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    format!(
        "{}?code=synthetic-code&state={}",
        query["return_url"], query["state"]
    )
}
pub(super) async fn action(
    state: &Arc<AppState>,
    view: &Value,
    grant: Option<&str>,
    action: &str,
    callback: Option<&str>,
) -> axum::response::Response {
    let mut body = json!({"expectedRevision":view["expectedRevision"]});
    if let Some(callback) = callback {
        body["callbackUrl"] = json!(callback);
    }
    support::send_console_json(
        state,
        Method::POST,
        &format!("{ROOT}/{}/{action}", view["id"].as_str().unwrap()),
        body,
        grant,
        None,
    )
    .await
}
pub(super) async fn complete(state: &Arc<AppState>, view: &Value, grant: &str, issuer: &Issuer) {
    let response = crate::protocol::accio_oauth::TEST_TOKEN_ENDPOINT
        .scope(
            issuer.endpoint.clone(),
            action(state, view, Some(grant), "complete", Some(&callback(view))),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = support::parse_json(response).await;
    assert_eq!(body["session"]["status"], "ready");
    for secret in [
        "synthetic-new-access",
        "synthetic-new-refresh",
        "synthetic-new-sg",
        "synthetic-device",
    ] {
        assert!(!body.to_string().contains(secret));
    }
}
