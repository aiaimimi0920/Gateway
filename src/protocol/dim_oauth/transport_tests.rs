//! Loopback sees only synthetic material; production authority validation still runs first.
use super::*;
use axum::{
    body::Body,
    http::{HeaderMap, Response, StatusCode},
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

struct Peer {
    origin: String,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    async fn serve(app: Router) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { origin, task }
    }
}
fn client() -> OAuthClient {
    OAuthClient::new(Site::China, "permitted-gateway", &["openid"]).unwrap()
}
fn http() -> rquest::Client {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert(
        "authorization",
        "Bearer inherited-must-not-send".parse().unwrap(),
    );
    headers.insert("x-api-key", "inherited-must-not-send".parse().unwrap());
    rquest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap()
}

#[tokio::test]
async fn dim_discovery_and_model_catalog_bind_audience_and_do_not_leak_shared_headers() {
    let seen = Arc::new(AtomicUsize::new(0));
    let model_seen = seen.clone();
    let app = Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(|headers: HeaderMap| async move {
                assert!(!headers.contains_key("authorization"));
                assert!(!headers.contains_key("x-api-key"));
                assert_eq!(headers["x-title"], "Neuro Gateway");
                Json(super::authorization_tests::document(Site::China))
            }),
        )
        .route(
            "/v1/models",
            get(move |headers: HeaderMap, query: axum::extract::RawQuery| {
                model_seen.fetch_add(1, Ordering::SeqCst);
                async move {
                    assert_eq!(headers["authorization"], "Bearer synthetic-account-access");
                    assert!(!headers.contains_key("x-api-key"));
                    assert!(!headers.contains_key("cookie"));
                    assert_eq!(query.0.as_deref(), Some("type=dim"));
                    Json(json!({"data":[{"id":"synthetic","dim":{"endpoint_type":"anthropic"}}]}))
                }
            }),
        );
    let peer = Peer::serve(app).await;
    let client = client();
    let http = http();
    super::transport::TEST_ORIGIN
        .scope(peer.origin.clone(), async {
            let metadata = client.discover(&http).await.unwrap();
            assert_eq!(metadata.client(), &client);
            assert_eq!(metadata.token_endpoint(), "https://dimagent.cn/oauth/token");
            let models = client
                .model_catalog(&http, "synthetic-account-access")
                .await
                .unwrap();
            assert_eq!(models[0].protocol, WireProtocol::Messages);
            assert_eq!(seen.load(Ordering::SeqCst), 1);
        })
        .await;
}

#[tokio::test]
async fn dim_metadata_redirect_and_account_denial_are_not_empty_catalog_success() {
    let sink = Arc::new(AtomicUsize::new(0));
    let tracker = sink.clone();
    let app = Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(|| async {
                Response::builder()
                    .status(StatusCode::FOUND)
                    .header("location", "/sink")
                    .body(Body::empty())
                    .unwrap()
            }),
        )
        .route(
            "/sink",
            get(move || {
                tracker.fetch_add(1, Ordering::SeqCst);
                async { "unexpected" }
            }),
        )
        .route(
            "/v1/models",
            get(|| async {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error":"synthetic-private-prose"})),
                )
            }),
        );
    let peer = Peer::serve(app).await;
    let client = client();
    let http = http();
    super::transport::TEST_ORIGIN
        .scope(peer.origin.clone(), async {
            assert!(matches!(
                client.discover(&http).await,
                Err(OAuthError::RedirectRejected)
            ));
            assert_eq!(sink.load(Ordering::SeqCst), 0);
            let error = client
                .model_catalog(&http, "synthetic-account-access")
                .await
                .unwrap_err();
            assert_eq!(error, OAuthError::ReauthorizationRequired);
            assert!(!error.to_string().contains("synthetic-private"));
        })
        .await;
}

#[tokio::test]
async fn dim_decoded_metadata_body_has_size_and_deadline_budgets() {
    let app = Router::new().route(
        "/.well-known/openid-configuration",
        get(|| async {
            let chunks = futures::stream::iter([
                Ok::<_, std::io::Error>(vec![b' '; 40_000]),
                Ok(vec![b' '; 40_000]),
            ]);
            Response::builder().body(Body::from_stream(chunks)).unwrap()
        }),
    );
    let peer = Peer::serve(app).await;
    let client = client();
    let http = http();
    let result = super::transport::TEST_ORIGIN
        .scope(peer.origin.clone(), client.discover(&http))
        .await;
    assert!(matches!(result, Err(OAuthError::ResponseTooLarge)));
    let app = Router::new().route(
        "/.well-known/openid-configuration",
        get(|| async {
            let chunks = futures::stream::pending::<Result<Vec<u8>, std::io::Error>>();
            Response::builder().body(Body::from_stream(chunks)).unwrap()
        }),
    );
    let slow = Peer::serve(app).await;
    let result = super::transport::TEST_ORIGIN
        .scope(
            slow.origin.clone(),
            super::transport::TEST_BUDGET.scope(Duration::from_millis(40), client.discover(&http)),
        )
        .await;
    assert!(matches!(result, Err(OAuthError::Timeout)));
}

#[tokio::test]
async fn dim_unauthorized_headers_retire_access_even_when_error_body_never_finishes() {
    let app = Router::new().route(
        "/v1/models",
        get(|| async {
            let chunks = futures::stream::pending::<Result<Vec<u8>, std::io::Error>>();
            Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::from_stream(chunks))
                .unwrap()
        }),
    );
    let peer = Peer::serve(app).await;
    let client = client();
    let http = http();
    let result = super::transport::TEST_ORIGIN
        .scope(
            peer.origin.clone(),
            super::transport::TEST_BUDGET.scope(
                Duration::from_millis(100),
                client.model_catalog(&http, "synthetic-access"),
            ),
        )
        .await;
    assert!(matches!(result, Err(OAuthError::ReauthorizationRequired)));
}
