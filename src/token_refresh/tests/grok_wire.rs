//! Real scheduler/HTTP/persistence with synthetic material and a scoped random-loopback issuer.
use super::*;
use crate::protocol::grok_oauth::{Authorization, AuthorizationContext, OAuthClient, Principal};
use axum::{
    http::{HeaderMap, StatusCode},
    response::Response,
};
use serde_json::{json, Value};
use std::sync::Mutex;

pub(super) struct Mock {
    pub issuer: String,
    pub calls: Arc<Mutex<Vec<(String, HeaderMap)>>>,
    server: Option<tokio::task::JoinHandle<()>>,
}
impl Mock {
    pub async fn new(status: u16, reply: Value, delay: u64) -> Self {
        let calls: Arc<Mutex<Vec<(String, HeaderMap)>>> = Arc::default();
        let captured = calls.clone();
        let app = axum::Router::new().route(
            "/oauth2/token",
            axum::routing::post(move |headers: HeaderMap, body: String| {
                let calls = captured.clone();
                let reply = reply.clone();
                async move {
                    calls.lock().unwrap().push((body, headers));
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    Response::builder()
                        .status(StatusCode::from_u16(status).unwrap())
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from(reply.to_string()))
                        .unwrap()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}", listener.local_addr().unwrap());
        let server = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap()
        }));
        Self {
            issuer,
            calls,
            server,
        }
    }
    pub fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
    pub async fn stop(mut self) {
        let task = self.server.take().unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
impl Drop for Mock {
    fn drop(&mut self) {
        if let Some(task) = &self.server {
            task.abort();
        }
    }
}
pub(super) async fn fixture() -> LocalFixture {
    let fixture = LocalFixture::new("https://never-requested.invalid/token").await;
    let snapshot = fixture.runtime.route_config().snapshot();
    fixture
        .runtime
        .commit_document(
            snapshot.revision().id(),
            serde_json::from_value(grok_context::document()).unwrap(),
            None,
        )
        .await
        .unwrap();
    fixture
}
pub(super) fn rotated() -> Value {
    json!({"access_token":"rotated-build-access","refresh_token":"rotated-build-refresh","expires_in":3600})
}
pub(super) async fn refresh(fixture: &LocalFixture, mock: &Mock, pending: &mut Pending) {
    let mut defaults = rquest::header::HeaderMap::new();
    defaults.insert("authorization", "Bearer must-not-leak".parse().unwrap());
    defaults.insert("cookie", "must-not-leak".parse().unwrap());
    let http = rquest::Client::builder()
        .no_proxy()
        .default_headers(defaults)
        .build()
        .unwrap();
    super::super::grok::TEST_ISSUER
        .scope(
            mock.issuer.clone(),
            refresh_once(
                fixture.runtime.route_config(),
                &http,
                Some(&fixture.runtime),
                None,
                pending,
                Duration::from_secs(300),
                None,
            ),
        )
        .await;
}

#[tokio::test]
async fn grok_scheduler_rotates_exact_client_principal_and_survives_restart() {
    let mock = Mock::new(200, rotated(), 0).await;
    let fixture = fixture().await;
    let mut pending = Pending::new();
    refresh(&fixture, &mock, &mut pending).await;
    assert!(pending.is_empty());
    let saved = fixture.saved();
    assert_eq!(saved.api_key.as_deref(), Some("rotated-build-access"));
    assert_eq!(
        saved.refresh_token.as_deref(),
        Some("rotated-build-refresh")
    );
    assert_eq!(
        serde_json::to_value(saved.grok_oauth.as_ref().unwrap()).unwrap(),
        grok_context::context()
    );
    let calls = mock.calls.lock().unwrap().clone();
    let form: HashMap<_, _> = url::form_urlencoded::parse(calls[0].0.as_bytes())
        .into_owned()
        .collect();
    assert_eq!(form.len(), 5);
    assert_eq!(form["grant_type"], "refresh_token");
    assert_eq!(form["client_id"], "synthetic-gateway-client");
    assert_eq!(form["principal_type"], "Team");
    assert_eq!(form["principal_id"], "synthetic-team");
    assert_eq!(form["refresh_token"], "synthetic-refresh");
    for name in ["cookie", "authorization", "x-xai-token-auth"] {
        assert!(!calls[0].1.contains_key(name));
    }
    let restarted = RouteConfigRuntime::new_local(&fixture.config).unwrap();
    restarted.recover_startup().await.unwrap();
    let snapshot = restarted.route_config().snapshot();
    let restored = &snapshot.document().providers[0].credentials[0];
    assert_eq!(restored.grok_oauth, saved.grok_oauth);
    assert_eq!(restored.expires_at, saved.expires_at);
    refresh(&fixture, &mock, &mut pending).await;
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn grok_refresh_denial_missing_expiry_scope_expansion_and_principal_conflict_do_not_apply() {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let changed_principal = format!(
        "{}.{}.sig",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(r#"{"principal_type":"Team","principal_id":"other-team"}"#)
    );
    for (status, body) in [
        (
            400,
            json!({"error":"invalid_grant","error_description":"private-upstream"}),
        ),
        (200, json!({"access_token":"without-expiry"})),
        (
            200,
            json!({"access_token":"scope-expanded","expires_in":60,"scope":"admin"}),
        ),
        (
            200,
            json!({"access_token":changed_principal,"expires_in":60}),
        ),
        (
            302,
            json!({"access_token":"redirect-must-not-apply","expires_in":60}),
        ),
    ] {
        let mock = Mock::new(status, body, 0).await;
        let fixture = fixture().await;
        let mut pending = Pending::new();
        refresh(&fixture, &mock, &mut pending).await;
        refresh(&fixture, &mock, &mut pending).await;
        assert_eq!(
            mock.count(),
            1,
            "failure must back off or require authorization"
        );
        assert_eq!(
            fixture.saved().api_key.as_deref(),
            Some("synthetic-build-token")
        );
        assert!(pending.is_empty());
        mock.stop().await;
    }
}

#[tokio::test]
async fn grok_receipt_snapshot_separates_consent_from_actual_endpoint_principal() {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let access = format!(
        "{}.{}.sig",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(r#"{"principal_type":"Team","principal_id":"actual-team"}"#)
    );
    let mock = Mock::new(
        200,
        json!({"access_token":access,"expires_in":60,"scope":"profile"}),
        0,
    )
    .await;
    let client = OAuthClient::new("explicit-client", &["openid", "profile"])
        .unwrap()
        .with_principal(Principal::new("User", "consent-user").unwrap())
        .test_issuer(&mock.issuer);
    let auth = Authorization::new(&client, "http://127.0.0.1:34567/callback").unwrap();
    let state = url::Url::parse(&auth.authorize_url())
        .unwrap()
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    let receipt = client
        .complete_code(
            &client_http(),
            &auth,
            &format!("{}?code=synthetic-code&state={state}", auth.redirect_uri()),
            None,
        )
        .await
        .unwrap();
    let context = AuthorizationContext::from_receipt(&receipt).unwrap();
    let value = serde_json::to_value(&context).unwrap();
    assert_eq!(value["issuer"], "https://auth.x.ai");
    assert_eq!(value["requested_principal"]["id"], "consent-user");
    assert_eq!(value["principal"]["id"], "actual-team");
    assert_eq!(value["requested_scopes"], json!(["openid", "profile"]));
    assert_eq!(value["granted_scopes"], json!(["profile"]));
    assert!(!format!("{context:?}").contains("actual-team"));
    mock.stop().await;
}
fn client_http() -> rquest::Client {
    super::client()
}
