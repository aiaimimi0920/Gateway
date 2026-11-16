//! Only cfg(test) can select a synthetic issuer; actual actions still traverse the normal router.
use super::*;
use crate::{
    console::grok_oauth::{Session, SessionStatus},
    http::router::build_router,
    routing::config::RouteConfigYaml,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};
use tower::ServiceExt;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
const ROOT: &str = "/v1/internal/gateway/console/grok-auth-sessions";

struct Issuer {
    client: OAuthClient,
    token: String,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Issuer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Issuer {
    async fn new(refresh: bool) -> Self {
        Self::with_lifetime(refresh, 120).await
    }
    async fn with_lifetime(refresh: bool, lifetime: u64) -> Self {
        let token = format!(
            "{}.{}.s",
            URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#),
            URL_SAFE_NO_PAD.encode(r#"{"principal_type":"Team","principal_id":"synthetic-team"}"#)
        );
        let mut body = json!({"access_token":token,"expires_in":lifetime,"scope":"openid"});
        if refresh {
            body["refresh_token"] = json!("synthetic-new-refresh");
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = OAuthClient::new("synthetic-explicit-client", &["openid", "offline_access"])
            .unwrap()
            .test_issuer(&format!("http://{}", listener.local_addr().unwrap()));
        let app = Router::new().route(
            "/oauth2/token",
            post(
                move |form: axum::extract::Form<std::collections::HashMap<String, String>>| {
                    let body = body.clone();
                    async move {
                        assert_eq!(form.0["client_id"], "synthetic-explicit-client");
                        assert_eq!(form.0["grant_type"], "authorization_code");
                        Json(body)
                    }
                },
            ),
        );
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            client,
            token,
            task,
        }
    }
}
fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"build","preset":"grok-oauth",
        "base_url":"https://cli-chat-proxy.grok.com/v1","explicit_credentials_only":true,"credentials":[]},
        {"id":"other","preset":"xai-api","base_url":"https://api.x.ai/v1","api_key":"synthetic-other-key"}]})).unwrap()
}
fn request(
    id: &str,
    action: &str,
    revision: &str,
    grant: &str,
    callback: Option<&str>,
) -> Request<Body> {
    let mut body = json!({"expectedRevision":revision});
    if let Some(callback) = callback {
        body["callbackUrl"] = json!(callback);
    }
    Request::post(format!("{ROOT}/{id}/{action}"))
        .header("x-management-token", support::MANAGEMENT_TOKEN)
        .header("x-secret-grant", grant)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
fn create(state: &Arc<AppState>, client: OAuthClient, credential: Option<&str>) -> Arc<Session> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-management-token",
        support::MANAGEMENT_TOKEN.parse().unwrap(),
    );
    let (owner, _) = authorize(state, &headers, None, None, false).unwrap();
    let revision = state.route_config.snapshot().revision().id().to_owned();
    let mut binding = Binding::new(&owner, "build", &revision).unwrap();
    if let Some(id) = credential {
        binding = binding.with_credential(id).unwrap();
    }
    state
        .lifecycle
        .grok_oauth
        .create(binding, client, "http://127.0.0.1:34567/callback", None)
        .unwrap()
}
async fn complete(state: &Arc<AppState>, session: &Arc<Session>, grant: &str) {
    let url = url::Url::parse(&session.view().authorization_url.unwrap()).unwrap();
    let pairs: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    let callback = format!(
        "{}?code=synthetic-code&state={}",
        pairs["redirect_uri"], pairs["state"]
    );
    let response = build_router(state.clone())
        .oneshot(request(
            session.id(),
            "complete",
            session.binding().revision(),
            grant,
            Some(&callback),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = support::parse_json(response).await;
    assert_eq!(body["session"]["status"], "ready");
    assert!(!body.to_string().contains("synthetic-new-refresh"));
}
async fn import(state: &Arc<AppState>, session: &Arc<Session>, grant: &str) -> Value {
    let response = build_router(state.clone())
        .oneshot(request(
            session.id(),
            "import",
            session.binding().revision(),
            grant,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    support::parse_json(response).await
}

#[tokio::test]
async fn normal_router_saves_receipt_with_context_secrets_original_expiry_and_idempotent_result() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let issuer = Issuer::new(true).await;
    let session = create(state, issuer.client.clone(), None);
    let grant = support::grant_console_secret_access(state).await;
    let before = time::OffsetDateTime::now_utc().unix_timestamp();
    complete(state, &session, &grant).await;
    let after = time::OffsetDateTime::now_utc().unix_timestamp();
    let lock = state.lifecycle.health_control.publication.lock().await;
    let body = import(state, &session, &grant).await;
    assert_eq!(body["session"]["status"], "importing");
    // A second HTTP caller and cancel must not take ownership of the accepted commit.
    assert_eq!(
        import(state, &session, &grant).await["session"]["status"],
        "importing"
    );
    let response = build_router(state.clone())
        .oneshot(request(
            session.id(),
            "cancel",
            session.binding().revision(),
            &grant,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(
        support::parse_json(response).await["session"]["status"],
        "importing"
    );
    drop(lock);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while session.view().status == SessionStatus::Importing {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let body = import(state, &session, &grant).await;
    assert_eq!(body["session"]["status"], "succeeded");
    let snapshot = state.route_config.snapshot();
    assert_eq!(body["session"]["savedRevision"], snapshot.revision().id());
    let target = &snapshot.document().providers[0].credentials;
    assert_eq!(target.len(), 1);
    assert_eq!(
        body["session"]["credentialId"].as_str(),
        target[0].id.as_deref()
    );
    assert_eq!(target[0].api_key.as_deref(), Some(issuer.token.as_str()));
    assert_eq!(
        target[0].refresh_token.as_deref(),
        Some("synthetic-new-refresh")
    );
    assert_eq!(
        target[0].grok_oauth.as_ref().unwrap().client_id(),
        "synthetic-explicit-client"
    );
    let expiry = time::OffsetDateTime::parse(
        target[0].expires_at.as_ref().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap()
    .unix_timestamp();
    assert!((before + 120..=after + 120).contains(&expiry));
    assert_eq!(
        snapshot.document().providers[1].api_key,
        "synthetic-other-key"
    );
    let redacted = crate::console::secrets::redact_route_document(snapshot.document()).unwrap();
    let public = serde_json::to_string(&redacted).unwrap();
    assert!(!public.contains(&issuer.token));
    assert!(!public.contains("synthetic-new-refresh"));
    assert!(!body.to_string().contains(&issuer.token));
    state.lifecycle.grok_oauth.join().await;
}

#[tokio::test]
async fn explicit_reauthorization_preserves_identity_policy_and_clears_absent_refresh() {
    let mut doc = document();
    let target:crate::routing::config::ProviderCredentialYaml=serde_json::from_value(json!({"id":"stable-id",
        "api_key":"synthetic-old-access","enabled":false,"account_name":"operator-name","supported_models":["operator-model"],
        "grok_oauth":{"issuer":"https://auth.x.ai","client_id":"old-client","requested_scopes":["openid"]},
        "refresh_token":"synthetic-old-refresh"})).unwrap();
    doc.providers[0].credentials.push(target);
    let fixture = support::ConsoleStateFixture::new(doc, true);
    let issuer = Issuer::new(false).await;
    let session = create(&fixture.state, issuer.client.clone(), Some("stable-id"));
    let grant = support::grant_console_secret_access(&fixture.state).await;
    complete(&fixture.state, &session, &grant).await;
    import(&fixture.state, &session, &grant).await;
    fixture.state.lifecycle.grok_oauth.join().await;
    assert_eq!(session.view().status, SessionStatus::Succeeded);
    let snapshot = fixture.state.route_config.snapshot();
    let accounts = &snapshot.document().providers[0].credentials;
    assert_eq!(accounts.len(), 1);
    let target = &accounts[0];
    assert_eq!(target.id.as_deref(), Some("stable-id"));
    assert_eq!(target.enabled, Some(false));
    assert_eq!(target.account_name.as_deref(), Some("operator-name"));
    assert_eq!(target.supported_models, ["operator-model"]);
    assert!(target.refresh_token.is_none());
    assert_eq!(target.api_key.as_deref(), Some(issuer.token.as_str()));
}

mod failures;
