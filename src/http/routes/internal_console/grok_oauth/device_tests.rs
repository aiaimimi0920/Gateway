//! The real management create/poll/import route uses an issuer override available only in tests.
use super::*;
use axum::{
    http::{Method, StatusCode},
    routing::post,
    Form, Router,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
const ROOT: &str = "/v1/internal/gateway/console/grok-auth-sessions";
tokio::task_local! { static ISSUER: String; }
pub(super) fn client_for_test(client: OAuthClient) -> OAuthClient {
    ISSUER
        .try_with(|issuer| client.clone().test_issuer(issuer))
        .unwrap_or(client)
}
struct Peer {
    url: String,
    calls: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    async fn new() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let tracker = calls.clone();
        let app = Router::new()
            .route("/oauth2/device/code",post(move |Form(form): Form<HashMap<String,String>>| async move {
                assert_eq!(form.len(),3); assert_eq!(form["client_id"],"permitted-client"); assert_eq!(form["referrer"],"neuro-gateway");
                Json(json!({"device_code":"private-device","user_code":"TEST-CODE","verification_uri":"https://accounts.x.ai/device","expires_in":120,"interval":1}))
            }))
            .route("/oauth2/token",post(move |headers: axum::http::HeaderMap, Form(form): Form<HashMap<String,String>>| {
                tracker.fetch_add(1,Ordering::SeqCst);
                async move {
                    assert!(!headers.contains_key("x-management-token")); assert!(!headers.contains_key("authorization"));
                    assert_eq!(form["device_code"],"private-device"); assert_eq!(form["grant_type"],"urn:ietf:params:oauth:grant-type:device_code");
                    Json(json!({"access_token":"private-access","refresh_token":"private-refresh","expires_in":120,"scope":"offline_access"}))
                }
            }));
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { url, calls, task }
    }
}
fn document() -> crate::routing::config::RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"build","preset":"grok-oauth","base_url":"https://cli-chat-proxy.grok.com/v1","explicit_credentials_only":true,"credentials":[]}]})).unwrap()
}
async fn send(state: &Arc<AppState>, path: &str, body: Value, grant: Option<&str>) -> Response {
    support::send_console_json(state, Method::POST, path, body, grant, None).await
}

#[tokio::test]
async fn device_normal_router_keeps_grants_and_revision_and_saves_only_after_explicit_import() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let revision = state.route_config.snapshot().revision().id().to_owned();
    let input = json!({"providerId":"build","expectedRevision":revision,"clientId":"permitted-client","scopes":["offline_access"],"authorizationFlow":"device"});
    let denied = send(state, ROOT, input.clone(), None).await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let grant = support::grant_console_secret_access(state).await;
    for patch in [
        json!({"redirectUri":"http://127.0.0.1:34567/callback"}),
        json!({"principalSelection":{"kind":"Team","id":"hint"}}),
    ] {
        let mut bad = input.clone();
        bad.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert_eq!(
            send(state, ROOT, bad, Some(&grant)).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    let peer = Peer::new().await;
    let response = ISSUER
        .scope(peer.url.clone(), send(state, ROOT, input, Some(&grant)))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = support::parse_json(response).await;
    assert_eq!(
        view["session"]["deviceAuthorization"]["userCode"],
        "TEST-CODE"
    );
    assert_eq!(view["session"]["identityVerified"], false);
    assert!(!view.to_string().contains("private-"));
    let id = view["session"]["id"].as_str().unwrap();
    let path = format!("{ROOT}/{id}/poll");
    assert_eq!(
        send(state, &path, json!({"expectedRevision":revision}), None)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            state,
            &path,
            json!({"expectedRevision":"wrong"}),
            Some(&grant)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(peer.calls.load(Ordering::SeqCst), 0);
    let early = send(
        state,
        &path,
        json!({"expectedRevision":revision}),
        Some(&grant),
    )
    .await;
    assert_eq!(
        support::parse_json(early).await["session"]["status"],
        "waiting_user"
    );
    assert_eq!(peer.calls.load(Ordering::SeqCst), 0);
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let response = send(
        state,
        &path,
        json!({"expectedRevision":revision}),
        Some(&grant),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let ready = support::parse_json(response).await;
    assert_eq!(ready["session"]["status"], "ready");
    assert_eq!(ready["session"]["identityVerified"], false);
    assert!(!ready.to_string().contains("private-"));
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    assert_eq!(peer.calls.load(Ordering::SeqCst), 1);
    let import = format!("{ROOT}/{id}/import");
    assert_eq!(
        send(state, &import, json!({"expectedRevision":revision}), None)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            state,
            &import,
            json!({"expectedRevision":revision}),
            Some(&grant)
        )
        .await
        .status(),
        StatusCode::OK
    );
    state.lifecycle.grok_oauth.join().await;
    let snapshot = state.route_config.snapshot();
    let credentials = &snapshot.document().providers[0].credentials;
    assert_eq!(credentials.len(), 1);
    assert_eq!(credentials[0].api_key.as_deref(), Some("private-access"));
    assert_eq!(
        credentials[0].refresh_token.as_deref(),
        Some("private-refresh")
    );
    assert_eq!(
        credentials[0].grok_oauth.as_ref().unwrap().client_id(),
        "permitted-client"
    );
    // Dedicated context owns refresh; legacy endpoint/client fields must stay absent.
    assert!(credentials[0].refresh_endpoint.is_none());
    assert!(credentials[0].refresh_client_id.is_none());
    let refresh = snapshot
        .oauth_refresh_target("build", credentials[0].id.as_deref().unwrap())
        .unwrap()
        .1
        .refresh_config
        .as_ref()
        .unwrap();
    assert_eq!(
        refresh.refresh_endpoint(),
        crate::protocol::grok_oauth::TOKEN_ENDPOINT
    );
    assert_eq!(refresh.client_id(), "permitted-client");
}
