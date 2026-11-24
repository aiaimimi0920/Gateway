//! Real management router and multipart body, including admission and the previous 8 MiB boundary.
use super::{
    files_upload_tests::{created, record, upload},
    pipeline_fixture::{support::*, *},
    test_support::*,
};
use crate::http::router::build_router;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

fn form(metadata: Value, size: usize, extra: bool) -> Vec<u8> {
    let mut body = format!("--boundary\r\nContent-Disposition: form-data; name=\"metadata\"\r\n\r\n{metadata}\r\n--boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"ignored.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();
    body.extend(vec![7; size]);
    if extra {
        body.extend_from_slice(
            b"\r\n--boundary\r\nContent-Disposition: form-data; name=\"extra\"\r\n\r\nno",
        );
    }
    body.extend_from_slice(b"\r\n--boundary--\r\n");
    body
}
async fn request(
    f: &Fixture,
    grant: Option<&str>,
    metadata: Value,
    size: usize,
    extra: bool,
) -> axum::response::Response {
    let mut request = Request::post("/v1/internal/gateway/console/luma-agents-files")
        .header("x-management-token", MANAGEMENT_TOKEN)
        .header("content-type", "multipart/form-data; boundary=boundary");
    if let Some(grant) = grant {
        request = request.header("x-secret-grant", grant);
    }
    Box::pin(
        build_router(f.state.clone()).oneshot(
            request
                .body(Body::from(form(metadata, size, extra)))
                .unwrap(),
        ),
    )
    .await
    .unwrap()
}
fn metadata(f: &Fixture, size: usize) -> Value {
    json!({"providerId":"luma-pipeline","credentialId":"exact", "expectedRevision":f.state.route_config.snapshot().revision().id(),"upload":upload(size)})
}

#[tokio::test]
async fn files_presigned_management_large_multipart_completes_without_base64() {
    let size = 64 * 1024 * 1024 + 1;
    let server = Server::start(vec![
        Reply::json(201, created(size)),
        Reply::json(200, Value::Null),
        Reply::json(200, record(size)),
    ])
    .await;
    within(
        &server.origin,
        Box::pin(async {
            let f = Box::pin(Fixture::new(&server, 5)).await;
            let grant = Box::pin(grant_console_secret_access(&f.state)).await;
            let response =
                Box::pin(request(&f, Some(&grant), metadata(&f, size), size, false)).await;
            let status = response.status();
            assert_eq!(response.headers()["cache-control"], "no-store");
            let body = parse_json(response).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            assert_eq!(body["result"]["file"]["id"], ID);
            assert_eq!(body["credentialId"], "exact");
            assert!(!body.to_string().contains("signature"));
            let calls = server.calls();
            assert_eq!(calls.len(), 3);
            assert_eq!(calls[1].raw_body.len(), size);
            assert!(calls[1].raw_body.iter().all(|b| *b == 7));
            assert!(!calls[1].headers.contains_key("authorization"));
            assert_eq!(calls[0].headers["authorization"], format!("Bearer {KEY}"));
            f.finish().await;
        }),
    )
    .await;
    server.stop().await;
}

#[tokio::test]
async fn files_presigned_management_denies_bad_scope_multipart_and_ungranted_uploads() {
    let server = Server::start(vec![]).await;
    within(
        &server.origin,
        Box::pin(async {
            let f = Box::pin(Fixture::new(&server, 5)).await;
            let grant = Box::pin(grant_console_secret_access(&f.state)).await;
            for (field, value) in [
                ("credentialId", "disabled"),
                ("credentialId", "backup-exact"),
                ("providerId", "foreign"),
                ("expectedRevision", "old"),
            ] {
                let mut input = metadata(&f, 3);
                input[field] = json!(value);
                assert!(Box::pin(request(&f, Some(&grant), input, 3, false))
                    .await
                    .status()
                    .is_client_error());
            }
            assert!(Box::pin(request(&f, None, metadata(&f, 3), 3, false))
                .await
                .status()
                .is_client_error());
            for (size, extra) in [(2, false), (4, false), (3, true)] {
                assert!(
                    Box::pin(request(&f, Some(&grant), metadata(&f, 3), size, extra))
                        .await
                        .status()
                        .is_client_error()
                );
            }
            assert!(server.calls().is_empty());
            f.finish().await;
        }),
    )
    .await;
    server.stop().await;
}
