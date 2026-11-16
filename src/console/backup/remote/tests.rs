use super::*;

#[test]
fn destinations_reject_insecure_or_ambiguous_paths() {
    for url in [
        "http://example.com/backup",
        "https://user:secret@example.com/backup",
        "https://example.com/backup?token=secret",
        "file:///backup",
    ] {
        assert!(secure_url(url).is_err());
    }
    assert!(secure_url("https://nas.example.com/backups/gateway.ngbk").is_ok());
    for path in ["../backup", "/backup", "backup//file", "a\\b", "a/./b"] {
        assert!(object_path(path).is_err());
    }
    assert!(object_path("backups/gateway.ngbk").is_ok());
}

#[test]
fn only_versioned_encrypted_envelopes_enter_transport() {
    assert!(validate_envelope(br#"{"token":"fixture"}"#).is_err());
    let mut envelope = vec![0; 52];
    envelope[..8].copy_from_slice(b"NGBK\0\0\0\x01");
    assert!(validate_envelope(&envelope).is_ok());
    envelope[7] = 2;
    assert!(validate_envelope(&envelope).is_err());
}

async fn fixture() -> (
    String,
    tokio::task::JoinHandle<()>,
    std::sync::Arc<std::sync::atomic::AtomicUsize>,
) {
    use axum::{
        body::Body,
        extract::Request,
        http::{header, Response},
        routing::any,
        Router,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let writes = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&writes);
    let app = Router::new().fallback(any(move |request: Request| {
        let observed = Arc::clone(&observed);
        async move {
            assert!(request.headers().contains_key(header::AUTHORIZATION));
            if request.method() == "PUT" {
                assert_eq!(request.headers().get(header::IF_NONE_MATCH).unwrap(), "*");
                observed.fetch_add(1, Ordering::SeqCst);
            }
            let mut body = vec![0; 52];
            body[..8].copy_from_slice(b"NGBK\0\0\0\x01");
            Response::builder()
                .header(header::ETAG, "\"fixture-revision\"")
                .body(Body::from(body))
                .unwrap()
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (address, task, writes)
}

#[tokio::test]
async fn webdav_and_s3_transfer_encrypted_bytes_with_conditional_writes() {
    let (address, server, writes) = fixture().await;
    let webdav = Destination::Webdav {
        url: format!("{address}/backup.ngbk"),
        username: "fixture".into(),
        password: "fixture-password".into(),
    };
    let s3 = Destination::S3 {
        endpoint: address,
        region: "us-east-1".into(),
        bucket: "test-bucket".into(),
        key: "backup.ngbk".into(),
        access_key_id: "fixture-access-key".into(),
        secret_access_key: "fixture-secret".into(),
        session_token: None,
    };
    // Call only the internal transport with loopback HTTP; production admission requires HTTPS.
    let first = webdav::download(&webdav).await.unwrap();
    assert_eq!(first.revision, "\"fixture-revision\"");
    webdav::upload(&webdav, first.bytes.clone(), None)
        .await
        .unwrap();
    let second = s3::download(&s3).await.unwrap();
    assert_eq!(second.bytes, first.bytes);
    s3::upload(&s3, second.bytes, None).await.unwrap();
    assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 2);
    server.abort();
    let _ = server.await;
}
