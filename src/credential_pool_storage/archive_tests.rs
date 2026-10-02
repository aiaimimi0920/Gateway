//! End-to-end archive/refill ownership using synthetic loopback storage only.
use super::*;
use crate::routing::config::ProviderConfigYaml;
use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::StatusCode,
    response::Response,
    Router,
};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

#[derive(Default)]
struct Objects {
    files: Mutex<HashMap<String, Vec<u8>>>,
    writes: AtomicUsize,
    reads: AtomicUsize,
    invalidate_on_read: AtomicUsize,
    revision_changed: AtomicBool,
    unknown_write: AtomicBool,
    reject_write: AtomicBool,
    fail_delete: AtomicBool,
    competing_write: Mutex<Option<Vec<u8>>>,
}
struct Fixture {
    endpoint: String,
    objects: Arc<Objects>,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn fixture() -> Fixture {
    async fn route(State(state): State<Arc<Objects>>, request: Request) -> Response {
        let path = request.uri().path().to_owned();
        let method = request.method().as_str().to_owned();
        let create_only = request
            .headers()
            .get("if-none-match")
            .is_some_and(|value| value == "*");
        let matched = request
            .headers()
            .get("if-match")
            .is_some_and(|v| v == "\"fixture-version\"");
        if method == "GET" {
            let read = state.reads.fetch_add(1, Ordering::SeqCst) + 1;
            if state.invalidate_on_read.load(Ordering::SeqCst) == read {
                state.revision_changed.store(true, Ordering::SeqCst);
            }
        }
        let (status, body) = match method.as_str() {
            "GET" => match state.files.lock().unwrap().get(&path).cloned() {
                Some(bytes) => (200, bytes),
                None => (404, Vec::new()),
            },
            "PUT" => {
                state.writes.fetch_add(1, Ordering::SeqCst);
                if state.reject_write.load(Ordering::SeqCst) {
                    (403, Vec::new())
                } else {
                    let body = to_bytes(request.into_body(), MAX_OBJECT_BYTES)
                        .await
                        .unwrap()
                        .to_vec();
                    let mut files = state.files.lock().unwrap();
                    if let Some(winner) = state.competing_write.lock().unwrap().take() {
                        files.insert(path.clone(), winner);
                    }
                    if !create_only || files.contains_key(&path) {
                        (412, Vec::new())
                    } else {
                        files.insert(path, body);
                        (
                            if state.unknown_write.load(Ordering::SeqCst) {
                                500
                            } else {
                                201
                            },
                            Vec::new(),
                        )
                    }
                }
            }
            "PROPFIND" => {
                let files = state.files.lock().unwrap();
                let entries: String = files.keys().filter(|key| key.starts_with(&path)).map(|key| format!("<d:response><d:href>{key}</d:href><d:propstat><d:prop><d:resourcetype/></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>")).collect();
                (
                    207,
                    format!("<d:multistatus xmlns:d=\"DAV:\">{entries}</d:multistatus>")
                        .into_bytes(),
                )
            }
            "DELETE" => {
                if !matched || state.fail_delete.load(Ordering::SeqCst) {
                    (412, Vec::new())
                } else {
                    state.files.lock().unwrap().remove(&path);
                    (204, Vec::new())
                }
            }
            "MKCOL" => (201, Vec::new()),
            _ => (405, Vec::new()),
        };
        Response::builder()
            .status(StatusCode::from_u16(status).unwrap())
            .header("ETag", "\"fixture-version\"")
            .body(Body::from(body))
            .unwrap()
    }
    let objects = Arc::new(Objects::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new().fallback(route).with_state(objects.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Fixture {
        endpoint,
        objects,
        server,
    }
}

fn provider(fixture: &Fixture) -> ProviderConfigYaml {
    serde_json::from_value(json!({"id":"provider-a","base_url":"https://example.invalid","credentials":[{"id":"synthetic","api_key":"synthetic-value"}], "credential_archive_connection":{"type":"webdav","endpoint":fixture.endpoint,"directory":"vault","username":"synthetic-user","password":"synthetic-password","allow_insecure_http":true}})).unwrap()
}
fn store(provider: &ProviderConfigYaml) -> RemoteStorage {
    let mut store = RemoteStorage::new(
        provider.credential_archive_connection.as_ref().unwrap(),
        &provider.id,
        true,
    )
    .unwrap();
    store.allow_fixture_capabilities();
    store
}

fn archive_path(provider: &ProviderConfigYaml, key: &str) -> String {
    format!(
        "/{}/{key}",
        namespace(
            provider.credential_archive_connection.as_ref().unwrap(),
            &provider.id,
            true
        )
    )
}
fn ids() -> HashSet<String> {
    HashSet::from(["synthetic".into()])
}

#[tokio::test]
async fn unknown_put_is_read_back_and_retry_never_duplicates_archive() {
    let fixture = fixture().await;
    let provider = provider(&fixture);
    let store = store(&provider);
    fixture.objects.unknown_write.store(true, Ordering::SeqCst);
    assert_eq!(
        archive::preserve_records(&store, &provider, &ids(), "r1-000000000000")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        archive::preserve_records(&store, &provider, &ids(), "r1-000000000000")
            .await
            .unwrap(),
        1
    );
    assert_eq!(fixture.objects.writes.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.objects.files.lock().unwrap().len(), 1);
    assert_eq!(
        archive::count_records(&store, &provider.id).await.unwrap(),
        1
    );
}

#[tokio::test]
async fn rejected_or_unowned_archive_never_verifies_source_removal() {
    let fixture = fixture().await;
    let provider = provider(&fixture);
    let original = serde_json::to_value(&provider.credentials).unwrap();
    let store = store(&provider);
    fixture.objects.reject_write.store(true, Ordering::SeqCst);
    assert!(
        archive::preserve_records(&store, &provider, &ids(), "r1-000000000000")
            .await
            .is_err()
    );
    assert_eq!(
        serde_json::to_value(&provider.credentials).unwrap(),
        original
    );
    assert!(fixture.objects.files.lock().unwrap().is_empty());
    let key = archive_record::revision_object_key(
        "synthetic",
        &provider.credentials[0],
        "r1-000000000000",
    )
    .unwrap();
    fixture.objects.files.lock().unwrap().insert(
        archive_path(&provider, &key),
        b"{\"ordinary\":true}".to_vec(),
    );
    fixture.objects.reject_write.store(false, Ordering::SeqCst);
    let before = fixture.objects.writes.load(Ordering::SeqCst);
    assert!(
        archive::preserve_records(&store, &provider, &ids(), "r1-000000000000")
            .await
            .is_err()
    );
    assert_eq!(fixture.objects.writes.load(Ordering::SeqCst), before);
}

#[tokio::test]
async fn count_and_purge_only_own_envelopes_and_conditional_delete_failure_is_safe() {
    let fixture = fixture().await;
    let provider = provider(&fixture);
    let store = store(&provider);
    archive::preserve_records(&store, &provider, &ids(), "r1-000000000000")
        .await
        .unwrap();
    let (own_path, bytes) = fixture
        .objects
        .files
        .lock()
        .unwrap()
        .iter()
        .next()
        .map(|(k, v)| (k.clone(), v.clone()))
        .unwrap();
    let mut foreign: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    foreign["providerId"] = json!("provider-b");
    {
        let mut files = fixture.objects.files.lock().unwrap();
        files.insert(
            archive_path(&provider, "foreign.json"),
            serde_json::to_vec(&foreign).unwrap(),
        );
        files.insert(
            archive_path(&provider, "ordinary.json"),
            b"{\"api_key\":\"synthetic\"}".to_vec(),
        );
        files.insert(archive_path(&provider, "copied.json"), bytes);
    }
    assert_eq!(
        archive::count_records(&store, &provider.id).await.unwrap(),
        1
    );
    fixture.objects.fail_delete.store(true, Ordering::SeqCst);
    assert!(
        super::purge::fixture_purge(self::store(&provider), &provider.id, &|| true)
            .await
            .is_err()
    );
    assert!(fixture
        .objects
        .files
        .lock()
        .unwrap()
        .contains_key(&own_path));
    fixture.objects.fail_delete.store(false, Ordering::SeqCst);
    assert_eq!(
        super::purge::fixture_purge(self::store(&provider), &provider.id, &|| true)
            .await
            .unwrap(),
        1
    );
    assert_eq!(fixture.objects.files.lock().unwrap().len(), 3);
    assert!(!fixture
        .objects
        .files
        .lock()
        .unwrap()
        .contains_key(&own_path));
}

#[tokio::test]
async fn remote_refill_is_provider_scoped_bounded_and_rejects_duplicate_selection() {
    let fixture = fixture().await;
    let provider = provider(&fixture);
    let connection = provider.credential_archive_connection.as_ref().unwrap();
    let prefix = namespace(connection, &provider.id, false);
    fixture.objects.files.lock().unwrap().insert(
        format!("/{prefix}/credential.json"),
        serde_json::to_vec(&provider.credentials[0]).unwrap(),
    );
    let credentials = refill::collect(connection, &provider.id, &[], 1)
        .await
        .unwrap();
    assert_eq!(credentials[0].id.as_deref(), Some("synthetic"));
    assert!(refill::collect(
        connection,
        &provider.id,
        &["credential.json".into(), "credential.json".into()],
        2
    )
    .await
    .is_err());
    assert!(
        refill::collect(connection, "provider-b", &["credential.json".into()], 1)
            .await
            .is_err()
    );
    fixture
        .objects
        .files
        .lock()
        .unwrap()
        .insert(format!("/{prefix}/large.json"), vec![b' '; 1024 * 1024 + 1]);
    assert!(
        refill::collect(connection, &provider.id, &["large.json".into()], 1)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn revision_change_before_delete_retains_archive() {
    let fixture = fixture().await;
    let provider = provider(&fixture);
    let store = store(&provider);
    archive::preserve_records(&store, &provider, &ids(), "r1-000000000000")
        .await
        .unwrap();
    fixture.objects.reads.store(0, Ordering::SeqCst);
    fixture
        .objects
        .invalidate_on_read
        .store(2, Ordering::SeqCst);
    let error = super::purge::fixture_purge(self::store(&provider), &provider.id, &|| {
        !fixture.objects.revision_changed.load(Ordering::SeqCst)
    })
    .await
    .unwrap_err();
    assert!(error.to_string().contains("revision changed"));
    assert_eq!(fixture.objects.files.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn intervening_archive_creator_wins_without_overwrite_and_retry_reads_back() {
    let fixture = fixture().await;
    let provider = provider(&fixture);
    let store = store(&provider);
    let record = archive_record::ArchiveRecord {
        schema_version: 2,
        source_revision: Some("r1-000000000000".into()),
        provider_id: provider.id.clone(),
        credential_id: "synthetic".into(),
        archived_at: "2026-10-01T00:00:00Z".into(),
        reason: "permanent_driver_rejection".into(),
        credential: provider.credentials[0].clone(),
    };
    let winner = serde_json::to_vec(&record).unwrap();
    *fixture.objects.competing_write.lock().unwrap() = Some(winner.clone());
    assert_eq!(
        archive::preserve_records(&store, &provider, &ids(), "r1-000000000000")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        fixture
            .objects
            .files
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap(),
        &winner
    );
    assert_eq!(fixture.objects.writes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn conflicting_unowned_archive_is_not_overwritten_or_accepted() {
    let fixture = fixture().await;
    let provider = provider(&fixture);
    let store = store(&provider);
    let winner = b"{\"ordinary\":true}".to_vec();
    *fixture.objects.competing_write.lock().unwrap() = Some(winner.clone());
    assert!(
        archive::preserve_records(&store, &provider, &ids(), "r1-000000000000")
            .await
            .is_err()
    );
    assert_eq!(
        fixture
            .objects
            .files
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap(),
        &winner
    );
    assert_eq!(provider.credentials.len(), 1);
}
