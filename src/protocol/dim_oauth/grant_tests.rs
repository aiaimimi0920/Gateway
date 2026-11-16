//! Real form exchange and userinfo acceptance use one owned code; no upstream prose escapes.
use super::*;
use axum::{
    extract::Form,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

pub(super) struct Peer {
    pub(super) origin: String,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    pub(super) async fn new(app: Router) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { origin, task }
    }
}
pub(super) fn grant() -> CodeGrant {
    let client = OAuthClient::new(
        Site::China,
        "permitted-gateway",
        &["openid", "profile", "email"],
    )
    .unwrap();
    let bytes = serde_json::to_vec(&super::authorization_tests::document(Site::China)).unwrap();
    let auth = Authorization::new(
        Metadata::parse(&client, &bytes).unwrap(),
        "http://127.0.0.1:54322/oauth/callback",
    )
    .unwrap();
    let callback = format!(
        "{}?state={}&code=synthetic-code",
        auth.redirect_uri(),
        auth.state
    );
    auth.into_code_grant(&callback).unwrap()
}
pub(super) fn token() -> Value {
    json!({"token_type":"Bearer","access_token":"synthetic-access","refresh_token":"synthetic-refresh","expires_in":120,"scope":"openid profile email"})
}

#[tokio::test]
async fn dim_code_grant_posts_pkce_without_shared_credentials_and_userinfo_establishes_site_local_identity(
) {
    let calls = Arc::new(AtomicUsize::new(0));
    let tracker = calls.clone();
    let app = Router::new().route("/oauth/token", post(move |headers: HeaderMap, Form(form): Form<HashMap<String,String>>| {
        tracker.fetch_add(1, Ordering::SeqCst);
        async move {
            assert!(!headers.contains_key("authorization")); assert!(!headers.contains_key("cookie"));
            assert_eq!(form.len(), 5); assert_eq!(form["client_id"], "permitted-gateway");
            assert_eq!(form["grant_type"], "authorization_code"); assert_eq!(form["code"], "synthetic-code");
            assert_eq!(form["redirect_uri"], "http://127.0.0.1:54322/oauth/callback");
            assert_eq!(form["code_verifier"].len(), 43); Json(token())
        }
    })).route("/oauth/userinfo", get(|headers: HeaderMap| async move {
        assert_eq!(headers["authorization"], "Bearer synthetic-access");
        Json(json!({"sub":"synthetic-sub","uid":"synthetic-uid","email":"synthetic@example.invalid"}))
    }));
    let peer = Peer::new(app).await;
    let http = rquest::Client::new();
    let receipt = super::transport::TEST_ORIGIN
        .scope(peer.origin.clone(), grant().exchange(&http))
        .await
        .unwrap();
    assert_eq!(receipt.subject(), "synthetic-sub");
    assert_eq!(receipt.user_id(), "synthetic-uid");
    assert_eq!(receipt.account_label(), Some("synthetic@example.invalid"));
    assert_eq!(
        receipt.identity_assurance(),
        IdentityAssurance::UserInfoEndpoint
    );
    assert_eq!(receipt.access_token(), "synthetic-access");
    assert_eq!(receipt.refresh_token(), "synthetic-refresh");
    assert!(!receipt.is_expired());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!format!("{receipt:?}").contains("synthetic"));
}

#[tokio::test]
async fn dim_invalid_id_token_is_not_ignored_and_does_not_fall_back_to_userinfo() {
    let users = Arc::new(AtomicUsize::new(0));
    let tracker = users.clone();
    let app = Router::new()
        .route(
            "/oauth/token",
            post(|| async {
                let mut value = token();
                value["id_token"] = json!("unsigned.invalid.token");
                Json(value)
            }),
        )
        .route(
            "/oauth/jwks.json",
            get(|| async { Json(json!({"keys":[]})) }),
        )
        .route(
            "/oauth/userinfo",
            get(move || {
                tracker.fetch_add(1, Ordering::SeqCst);
                async { Json(json!({"sub":"synthetic"})) }
            }),
        );
    let peer = Peer::new(app).await;
    let http = rquest::Client::new();
    let error = super::transport::TEST_ORIGIN
        .scope(peer.origin.clone(), grant().exchange(&http))
        .await
        .unwrap_err();
    assert_eq!(error, OAuthError::InvalidIdentity);
    assert_eq!(users.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn dim_rejected_code_is_not_replayed_or_turned_into_receipt() {
    for (status, body, expected) in [
        (
            StatusCode::BAD_REQUEST,
            json!({"error":"invalid_grant","error_description":"private-prose"}),
            OAuthError::ReauthorizationRequired,
        ),
        (
            StatusCode::FORBIDDEN,
            json!({"error":"client_not_allowed"}),
            OAuthError::Rejected,
        ),
        (
            StatusCode::TOO_MANY_REQUESTS,
            json!({"error":"rate_limit"}),
            OAuthError::Rejected,
        ),
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let tracker = calls.clone();
        let app = Router::new().route(
            "/oauth/token",
            post(move || {
                let body = body.clone();
                tracker.fetch_add(1, Ordering::SeqCst);
                async move { (status, Json(body)) }
            }),
        );
        let peer = Peer::new(app).await;
        let http = rquest::Client::new();
        let error = super::transport::TEST_ORIGIN
            .scope(peer.origin.clone(), grant().exchange(&http))
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(!error.to_string().contains("private-prose"));
    }
}

#[test]
fn dim_token_material_requires_bearer_refresh_and_real_lifetime_without_fabricated_defaults() {
    for patch in [
        json!({"token_type":"Basic"}),
        json!({"expires_in":0}),
        json!({"expires_in":null}),
        json!({"refresh_token":null}),
        json!({"scope":"openid openid"}),
        json!({"access_token":"bad\nvalue"}),
    ] {
        let mut body = token();
        body.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert!(
            super::grant::Tokens::parse(&serde_json::to_vec(&body).unwrap(), true, 1800000000)
                .is_err()
        );
    }
    let parsed =
        super::grant::Tokens::parse(&serde_json::to_vec(&token()).unwrap(), true, 1800000000)
            .unwrap();
    assert_eq!(parsed.expires_at_unix, 1800000120);
    assert!(!format!("{parsed:?}").contains("synthetic-access"));
}
