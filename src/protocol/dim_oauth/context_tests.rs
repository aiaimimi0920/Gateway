//! Persisted authority and scope ceilings survive a refresh without borrowing another product's grant.
use super::{
    grant_tests::{grant, token, Peer},
    *,
};
use axum::{extract::Form, http::HeaderMap, routing::post, Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;

pub(super) fn context() -> AuthorizationContext {
    let grant = grant();
    let tokens = super::grant::Tokens::parse(
        &serde_json::to_vec(&token()).unwrap(),
        true,
        super::grant::unix_now().unwrap(),
    )
    .unwrap();
    let receipt = GrantReceipt::accept(
        grant.authorization.metadata,
        tokens,
        br#"{"sub":"synthetic-subject","uid":"synthetic-user"}"#,
        None,
        grant.authorization.nonce,
    )
    .unwrap();
    AuthorizationContext::from_receipt(&receipt).unwrap()
}

#[test]
fn dim_context_roundtrip_pins_site_client_subject_endpoints_and_scope_ceiling() {
    let old = context();
    let value = serde_json::to_value(&old).unwrap();
    assert_eq!(
        serde_json::from_value::<AuthorizationContext>(value.clone()).unwrap(),
        old
    );
    assert!(old.permits_rotation(&old));
    assert!(!format!("{old:?}").contains("synthetic"));
    for (field, patch) in [
        ("site", json!("global")),
        ("client_id", json!("other-client")),
        ("subject", json!("another-subject")),
        ("nonce", json!("another-nonce")),
        ("user_id", json!("other-user")),
        (
            "token_endpoint",
            json!("https://dimagent.cn/oauth/other-token"),
        ),
        (
            "granted_scopes",
            json!(["openid", "profile", "email", "offline_access"]),
        ),
        ("granted_scopes", Value::Null),
    ] {
        let mut next = value.clone();
        next[field] = patch;
        let next: AuthorizationContext = serde_json::from_value(next).unwrap();
        assert!(!old.permits_rotation(&next), "{field}");
    }
    for (field, patch) in [
        (
            "token_endpoint",
            json!("https://foreign.invalid/oauth/token"),
        ),
        (
            "jwks_uri",
            json!("https://dimagent.cn/oauth/jwks.json?token=private"),
        ),
        ("subject", json!("bad\nsubject")),
        ("granted_scopes", json!(["openid", "openid"])),
    ] {
        let mut next = value.clone();
        next[field] = patch;
        assert!(serde_json::from_value::<AuthorizationContext>(next)
            .unwrap()
            .validate()
            .is_err());
    }
}

#[tokio::test]
async fn dim_refresh_uses_public_form_and_preserves_omitted_refresh_with_scope_narrowing() {
    let app = Router::new().route("/oauth/token", post(|headers: HeaderMap, Form(form): Form<HashMap<String,String>>| async move {
        assert!(!headers.contains_key("authorization")); assert!(!headers.contains_key("cookie"));
        assert_eq!(form.len(), 3); assert_eq!(form["grant_type"], "refresh_token");
        assert_eq!(form["client_id"], "permitted-gateway"); assert_eq!(form["refresh_token"], "synthetic-refresh");
        Json(json!({"access_token":"synthetic-successor","token_type":"Bearer","expires_in":80,"scope":"openid profile"}))
    }));
    let peer = Peer::new(app).await;
    let old = context();
    let http = rquest::Client::new();
    let received = super::transport::TEST_ORIGIN
        .scope(
            peer.origin.clone(),
            old.refresh_grant("synthetic-refresh")
                .unwrap()
                .execute(&http),
        )
        .await
        .unwrap();
    assert_eq!(received.refresh_token, "synthetic-refresh");
    assert_eq!(received.expires_in, 80);
    assert!(old.permits_rotation(&received.context));
    let scopes = serde_json::to_value(&received.context).unwrap();
    assert_eq!(scopes["granted_scopes"], json!(["openid", "profile"]));
    assert!(!format!("{received:?}").contains("synthetic-successor"));
}

#[tokio::test]
async fn dim_refresh_rejects_scope_expansion_and_malformed_rotation() {
    for patch in [
        json!({"scope":"openid profile email offline_access"}),
        json!({"refresh_token":""}),
        json!({"expires_in":0}),
    ] {
        let mut body = token();
        body.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let peer = Peer::new(Router::new().route(
            "/oauth/token",
            post(move || {
                let body = body.clone();
                async { Json(body) }
            }),
        ))
        .await;
        let http = rquest::Client::new();
        assert!(super::transport::TEST_ORIGIN
            .scope(
                peer.origin.clone(),
                context()
                    .refresh_grant("synthetic-refresh")
                    .unwrap()
                    .execute(&http)
            )
            .await
            .is_err());
    }
}
