//! Synthetic loopback contract tests. No cloud credentials or remote endpoints are used.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use tokio::task::JoinHandle;

use super::super::s3::S3Storage;
use super::super::{StorageNotFound, MAX_LIST_ENTRIES, MAX_OBJECT_BYTES};
use crate::routing::config::CredentialStorageConnection;

#[path = "s3_mutation_tests.rs"]
mod mutation;

const NAMESPACE: &str = "fixture/pool-a";

#[derive(Default)]
struct ObjectState {
    body: Mutex<Vec<u8>>,
    requests: Mutex<Vec<(Method, String, HeaderMap)>>,
    listings: Mutex<VecDeque<String>>,
    redirect: Mutex<Option<String>>,
    etag: Mutex<Option<String>>,
    delete_etag: Mutex<Option<String>>,
    ignore_conditions: AtomicBool,
}

struct Fixture {
    connection: CredentialStorageConnection,
    state: Arc<ObjectState>,
    server: JoinHandle<()>,
}

impl Fixture {
    fn storage(&self) -> S3Storage {
        let mut storage = S3Storage::new(&self.connection, NAMESPACE).unwrap();
        storage.allow_fixture_capabilities();
        storage
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn fixture() -> Fixture {
    let state = Arc::new(ObjectState::default());
    *state.etag.lock().unwrap() = Some("\"fixture-etag\"".into());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new()
        .fallback(serve_object)
        .with_state(state.clone());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    Fixture {
        connection: CredentialStorageConnection::S3 {
            endpoint: format!("http://{address}"),
            bucket: "fixture-bucket".into(),
            region: "auto".into(),
            prefix: "fixture".into(),
            access_key_id: Some("synthetic-access".into()),
            secret_access_key: Some("synthetic-secret".into()),
            session_token: Some("synthetic-session".into()),
            allow_insecure_http: true,
        },
        state,
        server,
    }
}

async fn serve_object(State(state): State<Arc<ObjectState>>, request: Request) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let query = request.uri().query().unwrap_or_default().to_owned();
    state.requests.lock().unwrap().push((
        method.clone(),
        request.uri().to_string(),
        request.headers().clone(),
    ));
    if let Some(target) = state.redirect.lock().unwrap().clone() {
        return (StatusCode::TEMPORARY_REDIRECT, [("location", target)]).into_response();
    }
    if path.ends_with("/slow.json") {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    if path.ends_with("/slow-body.json") {
        return Body::from_stream(futures::stream::once(async {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            Ok::<_, std::io::Error>("{}")
        }))
        .into_response();
    }
    if method == Method::GET
        && path.ends_with("/race.json")
        && state.body.lock().unwrap().is_empty()
    {
        // Another writer creates the object after our missing read but before our PUT.
        *state.body.lock().unwrap() = b"concurrent winner".to_vec();
        return StatusCode::NOT_FOUND.into_response();
    }
    if path.ends_with("/missing.json") {
        return StatusCode::NOT_FOUND.into_response();
    }
    if path.ends_with("/denied.json") {
        return (
            StatusCode::FORBIDDEN,
            "synthetic-secret synthetic-session private response",
        )
            .into_response();
    }
    if path.ends_with("/oversized.json") {
        return Body::from_stream(futures::stream::iter(
            (0..65).map(|_| Ok::<_, std::io::Error>(vec![b'x'; 65536])),
        ))
        .into_response();
    }
    if query.contains("list-type=2") {
        let xml = state
            .listings
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| listing(&[], None));
        if xml == "SLOW" {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        return ([("content-type", "application/xml")], xml).into_response();
    }
    match method {
        Method::PUT => {
            if !state.ignore_conditions.load(Ordering::SeqCst)
                && request
                    .headers()
                    .get("if-none-match")
                    .is_some_and(|value| value == "*")
                && !state.body.lock().unwrap().is_empty()
            {
                return StatusCode::PRECONDITION_FAILED.into_response();
            }
            let bytes = to_bytes(request.into_body(), MAX_OBJECT_BYTES)
                .await
                .unwrap()
                .to_vec();
            *state.body.lock().unwrap() = bytes;
            StatusCode::OK.into_response()
        }
        Method::GET => {
            let mut response = state.body.lock().unwrap().clone().into_response();
            if let Some(etag) = state.etag.lock().unwrap().as_ref() {
                response.headers_mut().insert("etag", etag.parse().unwrap());
            }
            response
        }
        Method::DELETE => {
            if let Some(if_match) = request
                .headers()
                .get("if-match")
                .filter(|_| !state.ignore_conditions.load(Ordering::SeqCst))
            {
                let current = state
                    .delete_etag
                    .lock()
                    .unwrap()
                    .clone()
                    .or_else(|| state.etag.lock().unwrap().clone());
                if current.as_deref() != if_match.to_str().ok() {
                    return StatusCode::PRECONDITION_FAILED.into_response();
                }
            }
            state.body.lock().unwrap().clear();
            StatusCode::NO_CONTENT.into_response()
        }
        _ => StatusCode::METHOD_NOT_ALLOWED.into_response(),
    }
}

fn listing(keys: &[&str], next: Option<&str>) -> String {
    let contents: String = keys
        .iter()
        .map(|key| format!("<Contents><Key>{key}</Key></Contents>"))
        .collect();
    let token = next
        .map(|token| format!("<NextContinuationToken>{token}</NextContinuationToken>"))
        .unwrap_or_default();
    format!("<ListBucketResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\"><IsTruncated>{}</IsTruncated>{contents}{token}</ListBucketResult>", next.is_some())
}

#[tokio::test]
async fn s3_signs_and_round_trips_scoped_objects() {
    let fixture = fixture().await;
    let storage = fixture.storage();
    let body = br#"{"secret":"synthetic-only"}"#;
    storage.put("account.json", body).await.unwrap();
    assert_eq!(storage.get("account.json").await.unwrap(), body);
    storage.delete("account.json").await.unwrap();
    let requests = fixture.state.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].2["if-none-match"], "*");
    for (_, uri, headers) in requests.iter() {
        assert_eq!(
            uri.split('?').next().unwrap(),
            "/fixture-bucket/fixture/pool-a/account.json"
        );
        let auth = headers["authorization"].to_str().unwrap();
        assert!(auth.starts_with("AWS4-HMAC-SHA256 Credential=synthetic-access/"));
        assert!(auth.contains("/auto/s3/aws4_request"));
        assert!(!auth.contains("synthetic-secret"));
        assert_eq!(headers["x-amz-security-token"], "synthetic-session");
    }
}

#[tokio::test]
async fn s3_lists_pages_and_filters_non_immediate_json_children() {
    let fixture = fixture().await;
    fixture.state.listings.lock().unwrap().extend([
        listing(
            &[
                "fixture/pool-a/b.json",
                "fixture/pool-a/sub/c.json",
                "fixture/pool-a/readme.txt",
            ],
            Some("next page"),
        ),
        listing(&["fixture/pool-a/a.json", "fixture/pool-a/b.json"], None),
    ]);
    assert_eq!(
        fixture.storage().list().await.unwrap(),
        ["a.json", "b.json"]
    );
    let requests = fixture.state.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let query: std::collections::HashMap<_, _> =
        url::form_urlencoded::parse(requests[1].1.split_once('?').unwrap().1.as_bytes()).collect();
    assert_eq!(query["prefix"], "fixture/pool-a/");
    assert_eq!(query["delimiter"], "/");
    assert_eq!(query["continuation-token"], "next page");
}

#[tokio::test]
async fn s3_rejects_escape_repeat_cursor_and_oversized_listing() {
    let fixture = fixture().await;
    for pages in [
        vec![listing(&["fixture/pool-other/a.json"], None)],
        vec![listing(&[], Some("cycle")), listing(&[], Some("cycle"))],
        vec![listing(
            &vec!["fixture/pool-a/a.json"; MAX_LIST_ENTRIES + 1],
            None,
        )],
        vec!["x".repeat(MAX_OBJECT_BYTES + 1)],
        (0..32)
            .map(|index| listing(&[], Some(&index.to_string())))
            .collect(),
    ] {
        *fixture.state.listings.lock().unwrap() = pages.into();
        assert!(fixture.storage().list().await.is_err());
    }
}

#[tokio::test]
async fn s3_errors_are_secret_safe_and_missing_is_typed() {
    let fixture = fixture().await;
    let storage = fixture.storage();
    let missing = storage.get("missing.json").await.unwrap_err();
    assert!(missing.is::<StorageNotFound>());
    let denied = storage.get("denied.json").await.unwrap_err();
    assert!(!denied.is::<StorageNotFound>());
    assert!(!format!("{denied:#?}").contains("synthetic"));
    assert!(!format!("{denied:#?}").contains("private response"));
    assert!(storage.get("oversized.json").await.is_err());
    assert!(storage
        .put("big.json", &vec![0; MAX_OBJECT_BYTES + 1])
        .await
        .is_err());
}

#[tokio::test]
async fn s3_never_follows_redirects_with_credentials_or_payloads() {
    let source = fixture().await;
    let target = fixture().await;
    let CredentialStorageConnection::S3 { endpoint, .. } = &target.connection else {
        unreachable!()
    };
    *source.state.redirect.lock().unwrap() = Some(format!("{endpoint}/stolen.json"));
    let storage = source.storage();
    assert!(storage.get("account.json").await.is_err());
    assert!(storage
        .put("account.json", b"private payload")
        .await
        .is_err());
    assert!(storage.list().await.is_err());
    assert!(storage.delete("account.json").await.is_err());
    assert!(target.state.requests.lock().unwrap().is_empty());
    assert_eq!(source.state.requests.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn s3_configuration_and_key_rejections_do_not_send_requests() {
    let mut fixture = fixture().await;
    let storage = fixture.storage();
    for key in [
        "../bad.json",
        "/absolute.json",
        "x%2Fy.json",
        "x\\y.json",
        "https://other/a.json",
    ] {
        assert!(storage.get(key).await.is_err());
        assert!(storage.put(key, b"{}").await.is_err());
        assert!(storage.delete(key).await.is_err());
    }
    for namespace in ["", "../other", "/other", "pool/%2e%2e"] {
        assert!(S3Storage::new(&fixture.connection, namespace).is_err());
    }
    assert!(fixture.state.requests.lock().unwrap().is_empty());
    let CredentialStorageConnection::S3 {
        allow_insecure_http,
        ..
    } = &mut fixture.connection
    else {
        unreachable!()
    };
    *allow_insecure_http = false;
    assert!(S3Storage::new(&fixture.connection, NAMESPACE).is_err());
    for endpoint in [
        "https://user:password@example.invalid",
        "https://example.invalid?token=secret",
        "https://example.invalid/#secret",
    ] {
        let CredentialStorageConnection::S3 {
            endpoint: configured,
            ..
        } = &mut fixture.connection
        else {
            unreachable!()
        };
        *configured = endpoint.into();
        assert!(S3Storage::new(&fixture.connection, NAMESPACE).is_err());
    }
}

#[tokio::test]
async fn s3_verified_delete_requires_bytes_strong_etag_and_server_match() {
    let fixture = fixture().await;
    let storage = fixture.storage();
    storage.put("account.json", b"original").await.unwrap();
    assert!(storage
        .delete_verified("account.json", b"different", "\"original\"", &|| true)
        .await
        .is_err());
    assert!(storage
        .delete_verified(
            "account.json",
            b"original",
            "\"different-version\"",
            &|| true
        )
        .await
        .is_err());
    for etag in [
        None,
        Some("W/\"weak\"".to_owned()),
        Some("unquoted".to_owned()),
    ] {
        *fixture.state.etag.lock().unwrap() = etag;
        assert!(storage
            .delete_verified("account.json", b"original", "\"original\"", &|| true)
            .await
            .is_err());
    }
    assert!(!fixture
        .state
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|(method, _, _)| *method == Method::DELETE));
    *fixture.state.etag.lock().unwrap() = Some("\"original\"".into());
    *fixture.state.delete_etag.lock().unwrap() = Some("\"replaced\"".into());
    assert!(storage
        .delete_verified("account.json", b"original", "\"original\"", &|| true)
        .await
        .is_err());
    assert_eq!(*fixture.state.body.lock().unwrap(), b"original");
    *fixture.state.delete_etag.lock().unwrap() = None;
    storage
        .delete_verified("account.json", b"original", "\"original\"", &|| true)
        .await
        .unwrap();
    let requests = fixture.state.requests.lock().unwrap();
    let deletes: Vec<_> = requests
        .iter()
        .filter(|(method, _, _)| *method == Method::DELETE)
        .collect();
    assert_eq!(deletes.len(), 2);
    assert!(deletes
        .iter()
        .all(|(_, _, headers)| headers["if-match"] == "\"original\""));
}

#[tokio::test]
async fn s3_connections_keep_auth_isolated_and_never_fall_back_to_environment() {
    let first = fixture().await;
    let mut connection = first.connection.clone();
    let CredentialStorageConnection::S3 {
        access_key_id,
        secret_access_key,
        session_token,
        ..
    } = &mut connection
    else {
        unreachable!()
    };
    *access_key_id = Some("second-access".into());
    *secret_access_key = Some("second-secret".into());
    *session_token = None;
    let second = S3Storage::new(&connection, "fixture/pool-b").unwrap();
    second.get("account.json").await.unwrap();
    first.storage().get("account.json").await.unwrap();
    let requests = first.state.requests.lock().unwrap();
    assert!(requests[0].2["authorization"]
        .to_str()
        .unwrap()
        .contains("Credential=second-access/"));
    assert!(requests[0].1.contains("/fixture/pool-b/"));
    assert!(!requests[0].2.contains_key("x-amz-security-token"));
    assert!(requests[1].2["authorization"]
        .to_str()
        .unwrap()
        .contains("Credential=synthetic-access/"));
    assert_eq!(requests[1].2["x-amz-security-token"], "synthetic-session");
    drop(requests);
    let CredentialStorageConnection::S3 {
        secret_access_key, ..
    } = &mut connection
    else {
        unreachable!()
    };
    *secret_access_key = None;
    assert!(S3Storage::new(&connection, NAMESPACE).is_err());
}

#[tokio::test]
async fn s3_deadlines_include_stalled_headers_bodies_and_listing() {
    let fixture = fixture().await;
    fixture
        .state
        .listings
        .lock()
        .unwrap()
        .push_back("SLOW".into());
    let mut storage = fixture.storage();
    storage.deadline = std::time::Duration::from_millis(50);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        let (headers, body, write, delete, listing) = tokio::join!(
            storage.get("slow.json"),
            storage.get("slow-body.json"),
            storage.put("slow.json", b"{}"),
            storage.delete("slow.json"),
            storage.list(),
        );
        for result in [
            headers.map(|_| ()),
            body.map(|_| ()),
            write,
            delete,
            listing.map(|_| ()),
        ] {
            assert!(result.unwrap_err().to_string().contains("timed out"));
        }
    })
    .await
    .expect("all storage operations must honor their network deadline");
}
