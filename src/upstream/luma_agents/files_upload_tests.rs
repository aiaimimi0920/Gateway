//! Presigned storage uses a different HTTP owner from the authenticated Luma API.
use super::{files::execute, files_tests::metadata, test_support::*};
use crate::protocol::luma_agents::{
    file_upload::{Upload, MAX_UPLOAD},
    files::{Operation, Prepared},
};
use bytes::Bytes;
use serde_json::{json, Value};

pub(super) fn upload(size: usize) -> Upload {
    serde_json::from_value(
        json!({"filename":"参考.png","mime_type":"image/png", "size_bytes":size,
        "purpose":"reference","expires_at":"2099-01-01T00:00:00Z","user_id":"owner-tag"}),
    )
    .unwrap()
}
pub(super) fn record(size: usize) -> Value {
    let mut file = metadata("pending");
    file["size_bytes"] = json!(size);
    file["expires_at"] = json!("2099-01-01T00:00:00Z");
    file["user_id"] = json!("owner-tag");
    file
}
pub(super) fn created(size: usize) -> Value {
    json!({"id":ID,"state":"pending","file":record(size),"upload": {
        "url":ASSET,"method":"PUT","expires_at":"2099-01-01T00:00:00Z","headers":{"Content-Type":"image/png"}}})
}
fn operation() -> Prepared {
    Operation::Presigned {
        metadata: upload(3),
        bytes: Bytes::from_static(&[1, 2, 3]).into(),
    }
    .prepare()
    .unwrap()
}

#[test]
fn files_presigned_validates_size_metadata_and_ttl_before_send() {
    assert!(upload(MAX_UPLOAD).validate().is_ok());
    for (key, value) in [
        ("size_bytes", json!(0)),
        ("size_bytes", json!(MAX_UPLOAD + 1)),
        ("expires_at", json!("2020-01-01T00:00:00Z")),
        ("expires_at", json!("bad")),
        ("user_id", json!("a\nsecret")),
        ("user_id", json!("x".repeat(257))),
        ("filename", json!("../a")),
        ("mime_type", json!("image/png\r\nCookie: x")),
        ("mime_type", json!("text/html")),
    ] {
        let mut data = serde_json::to_value(upload(3)).unwrap();
        data[key] = value;
        assert!(
            serde_json::from_value::<Upload>(data)
                .unwrap()
                .validate()
                .is_err(),
            "{key}"
        );
    }
    assert!(Operation::Presigned {
        metadata: upload(4),
        bytes: Bytes::from_static(&[1, 2, 3]).into()
    }
    .prepare()
    .is_err());
    assert!(serde_json::from_value::<Operation>(json!({"action":"presigned"})).is_err());
}

#[tokio::test]
async fn files_presigned_put_and_complete_bind_exact_bytes_id_and_credential_boundary() {
    let server = Server::start(vec![
        Reply::json(201, created(3)),
        Reply::json(200, Value::Null),
        Reply::json(200, record(3)),
    ])
    .await;
    let value = within(
        &server.origin,
        execute(&server.client(), &payload(), operation()),
    )
    .await
    .unwrap();
    assert_eq!(value["action"], "upload");
    assert_eq!(value["file"]["user_id"], "owner-tag");
    assert!(!value.to_string().contains("signature"));
    let calls = server.calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].body, serde_json::to_value(upload(3)).unwrap());
    assert_eq!(calls[1].method, "PUT");
    assert_eq!(calls[1].raw_body, [1, 2, 3]);
    assert_eq!(calls[1].headers["content-length"], "3");
    assert_eq!(calls[1].headers["content-type"], "image/png");
    for h in [
        "authorization",
        "cookie",
        "x-secret-grant",
        "x-management-token",
    ] {
        assert!(!calls[1].headers.contains_key(h));
    }
    assert_eq!(calls[2].path, format!("/v1/files/{ID}/complete"));
    assert_eq!(
        calls[2].headers["authorization"],
        "Bearer luma-api-synthetic-only"
    );
    server.stop().await;
}

#[tokio::test]
async fn files_presigned_rejects_unsafe_storage_expiry_headers_and_foreign_metadata() {
    for (pointer, value) in [
        ("/upload/url", json!("https://127.0.0.1/private")),
        ("/upload/url", json!("http://storage.test")),
        ("/upload/method", json!("POST")),
        ("/upload/expires_at", json!("2020-01-01T00:00:00Z")),
        ("/upload/headers", json!({"Authorization":"Bearer private"})),
        ("/upload/headers", json!({"Cookie":"private"})),
        ("/upload/headers", json!({"X-Amz-Copy-Source":"private"})),
        ("/upload/headers", json!({"Content-Type":"text/html"})),
        (
            "/upload/headers",
            json!({"Content-Type":"image/png","content-type":"image/png"}),
        ),
        ("/file/size_bytes", json!(99)),
        ("/file/user_id", json!("foreign")),
        ("/id", json!("foreign")),
    ] {
        let mut data = created(3);
        *data.pointer_mut(pointer).unwrap() = value;
        let server = Server::start(vec![Reply::json(201, data)]).await;
        let error = within(
            &server.origin,
            execute(&server.client(), &payload(), operation()),
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("private"));
        assert_eq!(server.calls().len(), 1, "{pointer}");
        server.stop().await;
    }
}

#[tokio::test]
async fn files_presigned_never_completes_failed_put_or_replays_uncertain_writes() {
    for status in [302, 403, 429, 500] {
        let server = Server::start(vec![
            Reply::json(201, created(3)),
            Reply::json(status, json!({"secret":"private"})),
        ])
        .await;
        let error = within(
            &server.origin,
            execute(&server.client(), &payload(), operation()),
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("private"));
        assert_eq!(server.calls().len(), 2);
        server.stop().await;
    }
    let mut foreign = record(3);
    foreign["id"] = json!("d290f1ee-6c54-4b01-90e6-d701748f0852");
    let server = Server::start(vec![
        Reply::json(201, created(3)),
        Reply::json(200, Value::Null),
        Reply::json(200, foreign),
    ])
    .await;
    assert!(within(
        &server.origin,
        execute(&server.client(), &payload(), operation())
    )
    .await
    .is_err());
    assert_eq!(server.calls().len(), 3);
    server.stop().await;
}

#[tokio::test]
async fn files_presigned_cancellation_drops_api_without_starting_put() {
    let server = Server::start(vec![Reply {
        status: 201,
        content_type: "application/json",
        bytes: vec![],
        hold: true,
    }])
    .await;
    within(&server.origin,async {
        let client=server.client(); let material=payload(); let work=execute(&client,&material,operation()); tokio::pin!(work);
        tokio::select! { _=&mut work=>panic!("request must wait"), _=tokio::time::sleep(std::time::Duration::from_millis(100))=>{} }
    }).await;
    server.wait_closed().await;
    assert_eq!(server.calls().len(), 1);
    server.stop().await;
}
