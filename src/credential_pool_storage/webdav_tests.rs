use super::*;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::Response as ServerResponse,
    Router,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::task::JoinHandle;

struct Reply {
    status: u16,
    body: Vec<u8>,
    location: Option<String>,
    delay: Duration,
    gzip: bool,
    etag: Option<String>,
}

impl Reply {
    fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            body: body.into(),
            location: None,
            delay: Duration::ZERO,
            gzip: false,
            etag: None,
        }
    }
}

struct Seen {
    method: String,
    path: String,
    headers: HeaderMap,
    body: Vec<u8>,
}

#[derive(Clone)]
struct Fixture {
    replies: Arc<Mutex<VecDeque<Reply>>>,
    seen: Arc<Mutex<Vec<Seen>>>,
}

struct Server {
    endpoint: String,
    fixture: Fixture,
    task: JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve(State(fixture): State<Fixture>, request: Request) -> ServerResponse {
    let (parts, body) = request.into_parts();
    let body = axum::body::to_bytes(body, MAX_OBJECT_BYTES + 1)
        .await
        .unwrap()
        .to_vec();
    fixture.seen.lock().unwrap().push(Seen {
        method: parts.method.to_string(),
        path: parts.uri.path().to_owned(),
        headers: parts.headers,
        body,
    });
    let reply = fixture
        .replies
        .lock()
        .unwrap()
        .pop_front()
        .unwrap_or_else(|| Reply::new(500, "unexpected request"));
    tokio::time::sleep(reply.delay).await;
    let mut response =
        ServerResponse::builder().status(StatusCode::from_u16(reply.status).unwrap());
    if let Some(location) = reply.location {
        response = response.header("Location", location);
    }
    if reply.gzip {
        response = response.header("Content-Encoding", "gzip");
    }
    if let Some(etag) = reply.etag {
        response = response.header("ETag", etag);
    }
    // Unknown-length chunked bodies exercise actual stream admission, not Content-Length.
    let chunks: Vec<_> = reply
        .body
        .chunks(8192)
        .map(|chunk| Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(chunk)))
        .collect();
    response
        .body(Body::from_stream(futures::stream::iter(chunks)))
        .unwrap()
}

async fn server(replies: Vec<Reply>) -> Server {
    let fixture = Fixture {
        replies: Arc::new(Mutex::new(replies.into())),
        seen: Arc::new(Mutex::new(Vec::new())),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/dav", listener.local_addr().unwrap());
    let router = Router::new().fallback(serve).with_state(fixture.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Server {
        endpoint,
        fixture,
        task,
    }
}

fn connection(server: &Server) -> CredentialStorageConnection {
    CredentialStorageConnection::Webdav {
        endpoint: server.endpoint.clone(),
        directory: "pool".to_owned(),
        username: Some("synthetic-user".to_owned()),
        password: Some("synthetic-password".to_owned()),
        allow_insecure_http: true,
    }
}

fn storage(server: &Server) -> WebdavStorage {
    let mut storage = WebdavStorage::new(&connection(server), "pool/provider").unwrap();
    storage.allow_fixture_capabilities();
    storage
}

#[tokio::test]
async fn authenticated_crud_uses_only_the_provider_namespace() {
    let listing = "<d:multistatus xmlns:d='DAV:'><d:response><d:href>/dav/pool/provider/a.json</d:href><d:status>HTTP/1.1 200 OK</d:status></d:response></d:multistatus>";
    let server = server(vec![
        Reply::new(207, listing),
        Reply::new(200, "{\"synthetic\":true}"),
        Reply::new(201, ""),
        Reply::new(204, ""),
    ])
    .await;
    let storage = storage(&server);
    assert_eq!(storage.list().await.unwrap(), vec!["a.json"]);
    assert_eq!(
        storage.get("a.json").await.unwrap(),
        b"{\"synthetic\":true}"
    );
    storage.put("b name.json", b"{}").await.unwrap();
    storage.delete("a.json").await.unwrap();
    let seen = server.fixture.seen.lock().unwrap();
    assert_eq!(
        seen.iter()
            .map(|request| request.method.as_str())
            .collect::<Vec<_>>(),
        vec!["PROPFIND", "GET", "PUT", "DELETE"]
    );
    for request in seen.iter() {
        assert_eq!(
            request.headers["authorization"],
            "Basic c3ludGhldGljLXVzZXI6c3ludGhldGljLXBhc3N3b3Jk"
        );
        assert!(request.path.starts_with("/dav/pool/provider/"));
    }
    assert_eq!(seen[0].headers["depth"], "1");
    assert_eq!(seen[2].path, "/dav/pool/provider/b%20name.json");
    assert_eq!(seen[2].body, b"{}");
}

#[tokio::test]
async fn missing_archive_creates_bounded_descendants_then_retries_put() {
    let server = server(vec![
        Reply::new(409, ""),
        Reply::new(405, ""),
        Reply::new(201, ""),
        Reply::new(201, ""),
    ])
    .await;
    storage(&server).put("a.json", b"{}").await.unwrap();
    let seen = server.fixture.seen.lock().unwrap();
    assert_eq!(
        seen.iter()
            .map(|request| (request.method.as_str(), request.path.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("PUT", "/dav/pool/provider/a.json"),
            ("MKCOL", "/dav/pool/"),
            ("MKCOL", "/dav/pool/provider/"),
            ("PUT", "/dav/pool/provider/a.json"),
        ]
    );
}

#[tokio::test]
async fn not_found_and_forbidden_remain_distinct_and_secret_safe() {
    let server = server(vec![
        Reply::new(404, "synthetic-password"),
        Reply::new(403, "synthetic-password"),
    ])
    .await;
    let storage = storage(&server);
    assert!(storage
        .get("a.json")
        .await
        .unwrap_err()
        .is::<StorageNotFound>());
    let error = storage.get("a.json").await.unwrap_err();
    assert!(!error.is::<StorageNotFound>());
    assert_eq!(error.to_string(), "WebDAV request returned HTTP 403");
    assert!(!format!("{error:?}").contains("synthetic"));
}

#[tokio::test]
async fn redirects_are_not_followed_or_given_credentials() {
    let mut reply = Reply::new(302, "");
    reply.location = Some("/untrusted-destination".to_owned());
    let server = server(vec![reply]).await;
    assert!(storage(&server)
        .get("a.json")
        .await
        .unwrap_err()
        .to_string()
        .contains("302"));
    assert_eq!(server.fixture.seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn network_timeout_is_bounded_and_secret_safe() {
    let mut reply = Reply::new(200, "{}");
    reply.delay = Duration::from_millis(200);
    let server = server(vec![reply]).await;
    let mut storage = storage(&server);
    storage.client = Client::builder()
        .redirect(rquest::redirect::Policy::none())
        .timeout(Duration::from_millis(30))
        .build()
        .unwrap();
    let error = storage.get("a.json").await.unwrap_err().to_string();
    assert_eq!(error, "WebDAV operation timed out");
}

#[tokio::test]
async fn stream_and_write_bounds_reject_oversized_objects() {
    let server = server(vec![Reply::new(200, vec![b'x'; MAX_OBJECT_BYTES + 1])]).await;
    let storage = storage(&server);
    assert!(storage
        .get("a.json")
        .await
        .unwrap_err()
        .to_string()
        .contains("size limit"));
    assert!(storage
        .put("a.json", &vec![b'x'; MAX_OBJECT_BYTES + 1])
        .await
        .is_err());
    assert_eq!(server.fixture.seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn decoded_gzip_bytes_are_bounded_independently_of_wire_size() {
    // Reproducible gzip(mtime=0) of 4 MiB + 1 synthetic 'x' bytes.
    let compressed = include_bytes!("webdav_oversized_response.gz");
    assert!(compressed.len() < 8192);
    let mut reply = Reply::new(200, compressed.to_vec());
    reply.gzip = true;
    let server = server(vec![reply]).await;
    assert!(storage(&server)
        .get("a.json")
        .await
        .unwrap_err()
        .to_string()
        .contains("size limit"));
}

#[tokio::test]
async fn invalid_keys_and_auth_are_rejected_before_network_io() {
    let server = server(vec![]).await;
    let storage = storage(&server);
    for key in ["../a.json", "nested/a.json", "a.txt", "a\\b.json", ""] {
        assert!(storage.get(key).await.is_err());
        assert!(storage.put(key, b"{}").await.is_err());
        assert!(storage.delete(key).await.is_err());
    }
    let mut config = connection(&server);
    if let CredentialStorageConnection::Webdav { username, .. } = &mut config {
        *username = None;
    }
    assert!(WebdavStorage::new(&config, "pool/provider").is_err());
    assert!(server.fixture.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn verified_delete_uses_if_match_and_never_falls_back_on_conflict() {
    for status in [204, 412] {
        let mut object = Reply::new(200, "{}");
        object.etag = Some("\"synthetic-version\"".to_owned());
        let server = server(vec![object, Reply::new(status, "")]).await;
        let result = storage(&server)
            .delete_verified("a.json", b"{}", "\"synthetic-version\"", &|| true)
            .await;
        assert_eq!(result.is_ok(), status == 204);
        let seen = server.fixture.seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[1].method, "DELETE");
        assert_eq!(seen[1].headers["if-match"], "\"synthetic-version\"");
    }
}

#[tokio::test]
async fn verified_delete_rejects_missing_weak_etags_and_changed_payloads() {
    for (etag, body) in [
        (None, "{}"),
        (Some("W/\"version\""), "{}"),
        (Some("*"), "{}"),
        (Some("\"version\""), "changed"),
        (Some("\"different-version\""), "{}"),
    ] {
        let mut object = Reply::new(200, body);
        object.etag = etag.map(str::to_owned);
        let server = server(vec![object]).await;
        assert!(storage(&server)
            .delete_verified("a.json", b"{}", "\"synthetic-version\"", &|| true)
            .await
            .is_err());
        assert_eq!(server.fixture.seen.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn unconfirmed_webdav_delete_never_reaches_a_server_that_would_ignore_preconditions() {
    let server = server(vec![Reply::new(204, "")]).await;
    let storage = WebdavStorage::new(&connection(&server), "pool/provider").unwrap();
    let error = storage
        .delete_verified("a.json", b"{}", "\"synthetic-version\"", &|| true)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("unconfirmed"));
    assert!(server.fixture.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn webdav_create_is_conditional_and_conflict_has_no_unconditional_retry() {
    let server = server(vec![Reply::new(412, "conflicting content")]).await;
    assert!(storage(&server).put("a.json", b"{}").await.is_err());
    let seen = server.fixture.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, "PUT");
    assert_eq!(seen[0].headers["if-none-match"], "*");
}
