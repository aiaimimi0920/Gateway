//! A complete code exchange must bind a valid signed ID token to the fixed userinfo subject.
use super::{
    grant_tests::{grant, token, Peer},
    *,
};
use axum::{
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/dim_oauth/rs256-vectors.json"
    ))
    .unwrap()
}
#[tokio::test]
async fn dim_signed_code_receipt_requires_jwks_signature_and_matching_userinfo_subject() {
    for matching in [true, false] {
        let vector = fixture();
        let keys = vector["jwks"].clone();
        let mut body = token();
        body["id_token"] = vector["valid"].clone();
        let app = Router::new()
            .route("/oauth/token", post(move || { let body = body.clone(); async { Json(body) } }))
            .route("/oauth/jwks.json", get(move || { let keys = keys.clone(); async { Json(keys) } }))
            .route("/oauth/userinfo", get(move || async move { Json(json!({"sub":if matching {"synthetic-subject"} else {"other-subject"},"uid":"synthetic-user"})) }));
        let peer = Peer::new(app).await;
        let http = rquest::Client::new();
        let mut code = grant();
        code.authorization.nonce = vector["nonce"].as_str().unwrap().into();
        let result = super::grant::TEST_UNIX_NOW
            .scope(
                vector["now"].as_u64().unwrap(),
                super::transport::TEST_ORIGIN.scope(peer.origin.clone(), code.exchange(&http)),
            )
            .await;
        if matching {
            let receipt = result.unwrap();
            assert_eq!(
                receipt.identity_assurance(),
                IdentityAssurance::SignedIdToken
            );
            let context = AuthorizationContext::from_receipt(&receipt).unwrap();
            assert_eq!(context.subject(), "synthetic-subject");
        } else {
            assert_eq!(result.unwrap_err(), OAuthError::IdentityMismatch);
        }
    }
}

#[tokio::test]
async fn dim_signed_refresh_checks_subject_and_keeps_a_single_total_deadline() {
    let vector = fixture();
    for matching in [true, false] {
        let keys = vector["jwks"].clone();
        let mut body = token();
        body["id_token"] = vector["valid"].clone();
        let peer = Peer::new(
            Router::new()
                .route(
                    "/oauth/token",
                    post(move || {
                        let body = body.clone();
                        async { Json(body) }
                    }),
                )
                .route(
                    "/oauth/jwks.json",
                    get(move || {
                        let keys = keys.clone();
                        async { Json(keys) }
                    }),
                ),
        )
        .await;
        let mut value = serde_json::to_value(super::context_tests::context()).unwrap();
        value["nonce"] = vector["nonce"].clone();
        if !matching {
            value["subject"] = json!("another-subject");
        }
        let context: AuthorizationContext = serde_json::from_value(value).unwrap();
        let http = rquest::Client::new();
        let result = super::grant::TEST_UNIX_NOW
            .scope(
                vector["now"].as_u64().unwrap(),
                super::transport::TEST_ORIGIN.scope(
                    peer.origin.clone(),
                    context
                        .refresh_grant("synthetic-refresh")
                        .unwrap()
                        .execute(&http),
                ),
            )
            .await;
        if matching {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.unwrap_err(), OAuthError::IdentityMismatch);
        }
    }
    let keys = vector["jwks"].clone();
    let mut body = token();
    body["id_token"] = vector["valid"].clone();
    let peer = Peer::new(
        Router::new()
            .route(
                "/oauth/token",
                post(move || {
                    let body = body.clone();
                    async {
                        tokio::time::sleep(std::time::Duration::from_millis(70)).await;
                        Json(body)
                    }
                }),
            )
            .route(
                "/oauth/jwks.json",
                get(move || {
                    let keys = keys.clone();
                    async {
                        tokio::time::sleep(std::time::Duration::from_millis(70)).await;
                        Json(keys)
                    }
                }),
            ),
    )
    .await;
    let http = rquest::Client::new();
    let result = super::grant::TEST_UNIX_NOW
        .scope(
            vector["now"].as_u64().unwrap(),
            super::transport::TEST_ORIGIN.scope(
                peer.origin.clone(),
                super::transport::TEST_BUDGET.scope(
                    std::time::Duration::from_millis(110),
                    super::context_tests::context()
                        .refresh_grant("synthetic-refresh")
                        .unwrap()
                        .execute(&http),
                ),
            ),
        )
        .await;
    assert_eq!(result.unwrap_err(), OAuthError::Timeout);
}

#[test]
fn dim_refresh_id_token_optional_nonce_must_match_original_authentication() {
    let vector = fixture();
    let client = OAuthClient::new(Site::China, "permitted-gateway", &["openid"]).unwrap();
    let keys = serde_json::to_vec(&vector["jwks"]).unwrap();
    let verify = |token: &str| {
        super::identity::verify_refresh(
            &client,
            token,
            "synthetic-access",
            "synthetic-nonce",
            &keys,
            1800000100,
        )
    };
    assert!(verify(vector["valid"].as_str().unwrap()).is_ok());
    assert!(verify(vector["refresh_without_nonce"].as_str().unwrap()).is_ok());
    let wrong = vector["rejected"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["reason"] == "nonce")
        .unwrap();
    assert!(verify(wrong["token"].as_str().unwrap()).is_err());
}
