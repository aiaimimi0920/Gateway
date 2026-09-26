use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aws_sdk_s3::config::{timeout::TimeoutConfig, Credentials, Region};
use axum::extract::{Query, State};
use axum::http::header::CONTENT_TYPE;
use axum::Router;
use tokio::task::JoinHandle;

use super::{GatewayObjectStorage, ObjectStorageDriver};

struct Pages {
    bodies: Vec<String>,
    requests: AtomicUsize,
    queries: Mutex<Vec<HashMap<String, String>>>,
}

struct Fixture {
    storage: GatewayObjectStorage,
    pages: Arc<Pages>,
    server: JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn fixture(bodies: Vec<String>) -> Fixture {
    assert!(!bodies.is_empty());
    let pages = Arc::new(Pages {
        bodies,
        requests: AtomicUsize::new(0),
        queries: Mutex::new(Vec::new()),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind S3 listing fixture");
    let address = listener.local_addr().expect("S3 fixture address");
    let router = Router::new().fallback(serve_page).with_state(pages.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("serve S3 listing fixture");
    });
    let config = aws_sdk_s3::config::Builder::new()
        .region(Region::new("us-east-1"))
        .credentials_provider(Credentials::new(
            "fixture-access",
            "fixture-secret",
            None,
            None,
            "fixture",
        ))
        .endpoint_url(format!("http://{address}"))
        .force_path_style(true)
        .behavior_version_latest()
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(Duration::from_millis(100))
                .read_timeout(Duration::from_millis(100))
                .operation_attempt_timeout(Duration::from_millis(100))
                .operation_timeout(Duration::from_millis(100))
                .build(),
        )
        .build();
    Fixture {
        storage: GatewayObjectStorage {
            driver: ObjectStorageDriver::S3Compatible {
                client: aws_sdk_s3::Client::from_conf(config),
                bucket: "fixture-bucket".to_string(),
                deadline: Duration::from_millis(100),
            },
        },
        pages,
        server,
    }
}

async fn serve_page(
    State(pages): State<Arc<Pages>>,
    Query(query): Query<HashMap<String, String>>,
) -> ([(axum::http::HeaderName, &'static str); 1], String) {
    let index = pages.requests.fetch_add(1, Ordering::SeqCst);
    if index < 4 {
        pages.queries.lock().expect("query ledger").push(query);
    }
    let body = pages.bodies[index.min(pages.bodies.len() - 1)].clone();
    ([(CONTENT_TYPE, "application/xml")], body)
}

fn page(truncated: bool, token: Option<&str>, key: &str) -> String {
    let token = token
        .map(|value| format!("<NextContinuationToken>{value}</NextContinuationToken>"))
        .unwrap_or_default();
    format!(
        "<ListBucketResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\">\
         <IsTruncated>{truncated}</IsTruncated>{token}\
         <Contents><Key>{key}</Key></Contents></ListBucketResult>"
    )
}

async fn assert_stalled(bodies: Vec<String>, expected_requests: usize) {
    let fixture = fixture(bodies).await;
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        fixture.storage.list_objects("prefix/"),
    )
    .await;
    let requests = fixture.pages.requests.load(Ordering::SeqCst);
    assert!(
        result.is_ok(),
        "stalled pagination did not terminate after {requests} requests"
    );
    let error = result
        .expect("pagination deadline")
        .expect_err("invalid pagination must return an error");
    assert_eq!(
        error.code.as_deref(),
        Some("object_storage_pagination_stalled")
    );
    assert_eq!(requests, expected_requests);
}

#[tokio::test]
async fn s3_listing_preserves_opaque_tokens_prefix_and_sorted_results() {
    let token = "opaque+/= value";
    let fixture = fixture(vec![
        page(true, Some(token), "zeta"),
        page(false, None, "alpha"),
    ])
    .await;
    let objects = tokio::time::timeout(
        Duration::from_secs(2),
        fixture.storage.list_objects("prefix/"),
    )
    .await
    .expect("valid pagination deadline")
    .expect("valid pagination");
    assert_eq!(objects, vec!["alpha", "zeta"]);
    let queries = fixture.pages.queries.lock().expect("query ledger");
    assert_eq!(queries.len(), 2);
    assert_eq!(queries[0].get("continuation-token"), None);
    assert_eq!(
        queries[1].get("continuation-token").map(String::as_str),
        Some(token)
    );
    for query in queries.iter() {
        assert_eq!(query.get("prefix").map(String::as_str), Some("prefix/"));
        assert_eq!(query.get("list-type").map(String::as_str), Some("2"));
    }
}

#[tokio::test]
async fn s3_listing_stops_when_a_truncated_page_omits_its_token() {
    assert_stalled(vec![page(true, None, "item")], 1).await;
}

#[tokio::test]
async fn s3_listing_stops_when_a_truncated_page_has_an_empty_token() {
    assert_stalled(vec![page(true, Some(""), "item")], 1).await;
}

#[tokio::test]
async fn s3_listing_stops_when_a_truncated_page_repeats_its_token() {
    assert_stalled(
        vec![
            page(true, Some("cursor"), "first"),
            page(true, Some("cursor"), "repeated"),
        ],
        2,
    )
    .await;
}

#[tokio::test]
async fn s3_listing_ignores_a_token_on_the_final_page() {
    let fixture = fixture(vec![page(false, Some("unused-cursor"), "item")]).await;
    let objects = tokio::time::timeout(
        Duration::from_secs(2),
        fixture.storage.list_objects("prefix/"),
    )
    .await
    .expect("final page deadline")
    .expect("final page");
    assert_eq!(objects, vec!["item"]);
    assert_eq!(fixture.pages.requests.load(Ordering::SeqCst), 1);
}
