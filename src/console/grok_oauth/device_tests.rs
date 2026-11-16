//! Device grants share the existing bounded registry and one-shot save receipt, never CLI state.
use super::*;
use crate::protocol::grok_oauth::{GrantAssurance, GrantFlow, OAuthClient, OAuthError};
use axum::{
    extract::State, http::StatusCode, response::IntoResponse, routing::post, Form, Json, Router,
};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};
type Calls = Arc<Mutex<Vec<HashMap<String, String>>>>;
struct Peer {
    client: OAuthClient,
    calls: Calls,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    async fn new(replies: Vec<(StatusCode, Value)>) -> Self {
        Self::with_interval(replies, 1).await
    }
    async fn with_interval(replies: Vec<(StatusCode, Value)>, interval: u64) -> Self {
        let calls = Calls::default();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = OAuthClient::new("permitted-client", &["offline_access"])
            .unwrap()
            .test_issuer(&format!("http://{}", listener.local_addr().unwrap()));
        let state = (calls.clone(), Arc::new(Mutex::new(VecDeque::from(replies))));
        async fn token(
            State((calls, replies)): State<(Calls, Arc<Mutex<VecDeque<(StatusCode, Value)>>>)>,
            Form(form): Form<HashMap<String, String>>,
        ) -> axum::response::Response {
            calls.lock().unwrap().push(form);
            let reply = replies.lock().unwrap().pop_front();
            match reply {
                Some((status, body)) => (status, Json(body)).into_response(),
                None => std::future::pending().await,
            }
        }
        let app = Router::new()
            .route("/oauth2/device/code", post(move || async move { Json(json!({"device_code":"private-device","user_code":"TEST-CODE","verification_uri":"https://accounts.x.ai/device","expires_in":120,"interval":interval})) }))
            .route("/oauth2/token", post(token))
            .with_state(state);
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            client,
            calls,
            task,
        }
    }
}
fn binding() -> Binding {
    Binding::new("owner", "build", "r1").unwrap()
}
fn http() -> rquest::Client {
    rquest::Client::builder().no_proxy().build().unwrap()
}
async fn wait_due(session: &Session) {
    tokio::time::sleep(Duration::from_secs(
        session.view().device_authorization.unwrap().poll_after_secs,
    ))
    .await;
}

#[tokio::test]
async fn device_pending_slowdown_and_receipt_keep_one_grant_without_claiming_signed_identity() {
    let peer = Peer::new(vec![
        (StatusCode::BAD_REQUEST,json!({"error":"authorization_pending"})),
        (StatusCode::BAD_REQUEST,json!({"error":"slow_down"})),
        (StatusCode::OK,json!({"access_token":"private-access","refresh_token":"private-refresh","expires_in":120})),
    ]).await;
    let registry = Registry::default();
    let session = registry
        .create_device(binding(), peer.client.clone(), None, &http())
        .await
        .unwrap();
    let view = session.view();
    assert_eq!(view.status, SessionStatus::WaitingUser);
    assert_eq!(
        view.device_authorization.as_ref().unwrap().user_code,
        "TEST-CODE"
    );
    assert!(!serde_json::to_string(&view).unwrap().contains("private-"));
    assert!(session
        .complete(&http(), "http://127.0.0.1:1/callback?code=x")
        .await
        .is_err());
    wait_due(&session).await;
    session.poll_device(&http()).await.unwrap();
    assert_eq!(session.view().status, SessionStatus::WaitingUser);
    wait_due(&session).await;
    session.poll_device(&http()).await.unwrap();
    assert_eq!(
        session
            .view()
            .device_authorization
            .unwrap()
            .poll_interval_secs,
        6
    );
    wait_due(&session).await;
    session.poll_device(&http()).await.unwrap();
    assert_eq!(session.view().status, SessionStatus::Ready);
    assert!(session.view().device_authorization.is_none());
    assert!(!session.view().identity_verified);
    assert!(session.poll_device(&http()).await.is_err());
    let receipt = session.claim_receipt(&binding()).unwrap();
    assert_eq!(receipt.flow(), GrantFlow::Device);
    assert_eq!(receipt.assurance(), GrantAssurance::TokenEndpointOnly);
    assert_eq!(
        receipt.tokens().refresh_token.as_deref(),
        Some("private-refresh")
    );
    assert!(session.claim_receipt(&binding()).is_err());
    for form in peer.calls.lock().unwrap().iter() {
        assert_eq!(form.len(), 3);
        assert_eq!(form["client_id"], "permitted-client");
        assert_eq!(form["device_code"], "private-device");
        assert_eq!(
            form["grant_type"],
            "urn:ietf:params:oauth:grant-type:device_code"
        );
    }
    registry.join().await;
}

#[tokio::test]
async fn device_capacity_is_reserved_before_network_and_code_session_cannot_be_polled() {
    let registry = Registry::default();
    let client = OAuthClient::new("permitted-client", &["offline_access"])
        .unwrap()
        .test_issuer("http://127.0.0.1:1");
    for _ in 0..16 {
        let code = registry
            .create(
                binding(),
                client.clone(),
                "http://127.0.0.1:34567/callback",
                None,
            )
            .unwrap();
        assert!(code.poll_device(&http()).await.is_err());
        assert_eq!(code.view().status, SessionStatus::WaitingUser);
    }
    assert!(matches!(
        registry
            .create_device(binding(), client, None, &http())
            .await,
        Err(LoginError::Capacity)
    ));
    registry.join().await;
}

#[tokio::test]
async fn device_cancellation_and_dropped_inflight_poll_fail_closed_without_replay() {
    for cancel in [true, false] {
        let peer = Peer::new(vec![]).await;
        let registry = Registry::default();
        let session = registry
            .create_device(binding(), peer.client.clone(), None, &http())
            .await
            .unwrap();
        wait_due(&session).await;
        let owned = session.clone();
        let task = tokio::spawn(async move { owned.poll_device(&http()).await });
        tokio::time::timeout(Duration::from_secs(1), async {
            while peer.calls.lock().unwrap().is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(session.poll_device(&http()).await.is_err());
        if cancel {
            session.cancel();
            assert!(task.await.unwrap().is_err());
            assert_eq!(session.view().status, SessionStatus::Cancelled);
        } else {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            assert_eq!(session.view().status, SessionStatus::Failed);
        }
        assert_eq!(peer.calls.lock().unwrap().len(), 1);
        assert!(session.view().authorization_url.is_none());
        assert!(session.claim_receipt(&binding()).is_err());
        registry.join().await;
    }
}

#[tokio::test]
async fn device_rejection_and_shutdown_cannot_leave_an_importable_receipt() {
    let peer = Peer::new(vec![(
        StatusCode::BAD_REQUEST,
        json!({"error":"access_denied"}),
    )])
    .await;
    let registry = Registry::default();
    let session = registry
        .create_device(binding(), peer.client.clone(), None, &http())
        .await
        .unwrap();
    wait_due(&session).await;
    assert!(matches!(
        session.poll_device(&http()).await,
        Err(LoginError::OAuth(OAuthError::AccessDenied))
    ));
    assert_eq!(session.view().status, SessionStatus::Failed);
    assert!(session.claim_receipt(&binding()).is_err());
    registry.join().await;
    assert!(matches!(
        registry
            .create_device(binding(), peer.client.clone(), None, &http())
            .await,
        Err(LoginError::Stopped)
    ));
}

#[tokio::test]
async fn device_long_poll_interval_returns_immediately_without_network_or_consuming_challenge() {
    let peer = Peer::with_interval(vec![], 60).await;
    let registry = Registry::default();
    let session = registry
        .create_device(binding(), peer.client.clone(), None, &http())
        .await
        .unwrap();
    for _ in 0..2 {
        tokio::time::timeout(Duration::from_millis(100), session.poll_device(&http()))
            .await
            .unwrap()
            .unwrap();
    }
    let view = session.view();
    assert_eq!(view.status, SessionStatus::WaitingUser);
    assert!(view.device_authorization.unwrap().poll_after_secs > 30);
    assert!(peer.calls.lock().unwrap().is_empty());
    registry.join().await;
}
