//! Synthetic fixed-authority peer; no real credentials or public network.
use super::*;
use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

pub(crate) fn token(subject: &str, expiry: i64) -> String {
    format!(
        "e30.{}.c2ln",
        URL_SAFE_NO_PAD.encode(json!({"sub":subject,"exp":expiry}).to_string())
    )
}
pub(crate) fn access() -> String {
    token("auth0|synthetic", 4070908800)
}
pub(crate) struct Peer {
    pub origin: String,
    pub login: Arc<Mutex<Value>>,
    pub identity: Arc<Mutex<Value>>,
    pub status: Arc<Mutex<StatusCode>>,
    pub polls: Arc<AtomicUsize>,
    pub checks: Arc<AtomicUsize>,
    pub release: Arc<tokio::sync::Notify>,
    task: tokio::task::JoinHandle<()>,
}
impl Peer {
    pub async fn new(block_identity: bool) -> Self {
        let login = Arc::new(Mutex::new(json!({"accessToken":access()})));
        let identity = Arc::new(Mutex::new(
            json!({"authId":"auth0|synthetic","email":"owner@example.test"}),
        ));
        let status = Arc::new(Mutex::new(StatusCode::OK));
        let polls = Arc::new(AtomicUsize::new(0));
        let checks = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(tokio::sync::Notify::new());
        let (l, p) = (login.clone(), polls.clone());
        let (i, c, s, r) = (
            identity.clone(),
            checks.clone(),
            status.clone(),
            release.clone(),
        );
        let router = Router::new()
            .route(
                "/auth/poll",
                get(
                    move |axum::extract::Query(query): axum::extract::Query<
                        std::collections::HashMap<String, String>,
                    >| {
                        let (l, p) = (l.clone(), p.clone());
                        async move {
                            assert!(uuid::Uuid::parse_str(&query["uuid"]).is_ok());
                            assert_eq!(query["verifier"].len(), 43);
                            p.fetch_add(1, Ordering::SeqCst);
                            Json(l.lock().clone())
                        }
                    },
                ),
            )
            .route(
                "/aiserver.v1.DashboardService/GetMe",
                post(move |headers: axum::http::HeaderMap, body: String| {
                    let (i, c, s, r) = (i.clone(), c.clone(), s.clone(), r.clone());
                    async move {
                        assert!(headers["authorization"]
                            .to_str()
                            .unwrap()
                            .starts_with("Bearer "));
                        assert_eq!(body, "{}");
                        c.fetch_add(1, Ordering::SeqCst);
                        if block_identity {
                            r.notified().await;
                        }
                        (
                            *s.lock(),
                            [
                                ("content-type", "application/json"),
                                ("location", "/forbidden"),
                            ],
                            i.lock().to_string(),
                        )
                            .into_response()
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            origin,
            login,
            identity,
            status,
            polls,
            checks,
            release,
            task,
        }
    }
    pub async fn checked(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while self.checks.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[test]
fn cursor_token_shape_expiry_and_duplicate_claims_are_rejected() {
    for value in [
        "",
        "opaque",
        "Bearer e30.e30.c2ln",
        "a.b.c.d",
        "a..c",
        "a.%.c",
    ] {
        assert!(claims(value).is_err());
    }
    let duplicate = format!(
        "e30.{}.c2ln",
        URL_SAFE_NO_PAD.encode(br#"{"sub":"a","sub":"b","exp":4070908800}"#)
    );
    assert!(claims(&duplicate).is_err());
    assert!(claims(&token("a\nb", 4070908800)).is_err());
    assert_eq!(require_fresh(1).unwrap_err().0, "cursor_token_expired");
    assert_eq!(claims(&access()).unwrap().sub, "auth0|synthetic");
}
#[test]
fn cursor_challenge_contains_only_pkce_public_values() {
    let challenge = Challenge::new().unwrap();
    let url = url::Url::parse(&challenge.url).unwrap();
    assert_eq!(url.origin().ascii_serialization(), "https://www.cursor.com");
    let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
    assert_eq!(params.len(), 4);
    assert_eq!(
        params["challenge"],
        URL_SAFE_NO_PAD.encode(Sha256::digest(challenge.verifier.as_bytes()))
    );
    assert!(!challenge.url.contains(&challenge.verifier));
    assert_eq!(params["redirectTarget"], "cli");
}
#[tokio::test]
async fn cursor_verify_requires_matching_subject_email_and_fresh_token() {
    let peer = Peer::new(false).await;
    TEST_ORIGIN
        .scope(peer.origin.clone(), async {
            let http = client().unwrap();
            assert!(verify(&http, access(), Some(" OWNER@example.test "))
                .await
                .is_ok());
            for identity in [
                json!({"authId":"other","email":"owner@example.test"}),
                json!({"authId":"auth0|synthetic","email":"other@example.test"}),
            ] {
                *peer.identity.lock() = identity;
                assert!(matches!(
                    verify(&http, access(), Some("owner@example.test")).await,
                    Err(AuthError("cursor_account_identity_mismatch"))
                ));
            }
            let count = peer.checks.load(Ordering::SeqCst);
            assert!(matches!(
                verify(&http, token("auth0|synthetic", 1), None).await,
                Err(AuthError("cursor_token_expired"))
            ));
            assert_eq!(peer.checks.load(Ordering::SeqCst), count);
        })
        .await;
}
#[tokio::test]
async fn cursor_verify_discovers_email_without_relaxing_remote_identity_validation() {
    let peer = Peer::new(false).await;
    TEST_ORIGIN
        .scope(peer.origin.clone(), async {
            let http = client().unwrap();
            peer.identity.lock()["email"] = json!(" Discovered@example.test ");
            let material = verify(&http, access(), None).await.unwrap();
            assert_eq!(material.email, "discovered@example.test");
            assert_eq!(material.subject, "auth0|synthetic");
            for email in ["", "invalid", "owner @example.test"] {
                peer.identity.lock()["email"] = json!(email);
                assert!(matches!(
                    verify(&http, access(), None).await,
                    Err(AuthError("cursor_invalid_identity_response"))
                ));
            }
            *peer.identity.lock() = json!({"authId":"other","email":"owner@example.test"});
            assert!(matches!(
                verify(&http, access(), None).await,
                Err(AuthError("cursor_account_identity_mismatch"))
            ));
        })
        .await;
}
#[tokio::test]
async fn cursor_poll_accepts_alias_but_rejects_ambiguous_material_and_redirects() {
    let peer = Peer::new(false).await;
    TEST_ORIGIN
        .scope(peer.origin.clone(), async {
            let http = client().unwrap();
            let challenge = Challenge::new().unwrap();
            *peer.login.lock() = json!({"apiKey":access()});
            assert_eq!(poll(&http, &challenge).await.unwrap(), Some(access()));
            *peer.login.lock() = json!({"apiKey":"different","accessToken":access()});
            assert!(matches!(
                poll(&http, &challenge).await,
                Err(AuthError("cursor_ambiguous_login_token"))
            ));
            *peer.status.lock() = StatusCode::TEMPORARY_REDIRECT;
            assert!(matches!(
                verify(&http, access(), None).await,
                Err(AuthError("cursor_redirect_rejected"))
            ));
            assert_eq!(peer.checks.load(Ordering::SeqCst), 1);
        })
        .await;
}
#[tokio::test]
async fn cursor_identity_response_is_bounded_and_errors_do_not_echo_secrets() {
    let peer = Peer::new(false).await;
    *peer.identity.lock() = json!({"secret": "x".repeat(65537)});
    TEST_ORIGIN
        .scope(peer.origin.clone(), async {
            let result = verify(&client().unwrap(), access(), None).await;
            assert!(matches!(
                result,
                Err(AuthError("cursor_response_too_large"))
            ));
        })
        .await;
}
