//! Files contract at the real HTTP boundary, without vendor accounts or external storage.
use super::{files::execute, test_support::*};
use crate::protocol::luma_agents::files::{Operation, Prepared, MAX_UPLOAD};
use axum::{
    body::Body,
    extract::{FromRequest, Multipart},
    http::Request,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::time::Duration;

pub(super) fn metadata(state: &str) -> Value {
    json!({"id":ID,"filename":"参考.png","mime_type":"image/png","size_bytes":3,
        "purpose":"reference","state":state,"created_at":"2026-11-20T00:00:00Z"})
}
fn prepare(value: Value) -> Prepared {
    serde_json::from_value::<Operation>(value)
        .unwrap()
        .prepare()
        .unwrap()
}

#[test]
fn files_input_rejects_paths_malformed_base64_and_oversize() {
    for filename in ["", "../a.png", "a\\b.png", "a\r\nInjected: true", "a\".png"] {
        assert!(
            serde_json::from_value::<Operation>(json!({"action":"upload","filename":filename,
            "purpose":"input","data":"AQID"}))
            .unwrap()
            .prepare()
            .is_err()
        );
    }
    for data in [
        "".into(),
        "not-base64".into(),
        STANDARD.encode(vec![1; MAX_UPLOAD + 1]),
    ] {
        assert!(
            serde_json::from_value::<Operation>(json!({"action":"upload","filename":"a.png",
            "purpose":"input","data":data}))
            .unwrap()
            .prepare()
            .is_err()
        );
    }
    for value in [
        json!({"action":"get","id":"../foreign"}),
        json!({"action":"list","cursor":"bad\nvalue"}),
    ] {
        assert!(serde_json::from_value::<Operation>(value)
            .unwrap()
            .prepare()
            .is_err());
    }
    assert!(serde_json::from_value::<Operation>(
        json!({"action":"list","url":"https://foreign.test"})
    )
    .is_err());
    assert!(serde_json::from_value::<Operation>(
        json!({"action":"upload","filename":"a.png","purpose":"other","data":"AQID"})
    )
    .is_err());
}

#[tokio::test]
async fn files_rejects_wrong_product_material_before_network() {
    let server = Server::start(vec![]).await;
    for field in ["base_url", "api_key", "adapter"] {
        let mut material = serde_json::to_value(payload()).unwrap();
        material[field] = json!("foreign");
        let material = serde_json::from_value(material).unwrap();
        assert!(within(
            &server.origin,
            execute(&server.client(), &material, Prepared::List(None))
        )
        .await
        .is_err());
    }
    assert!(server.calls().is_empty());
    server.stop().await;
}

#[tokio::test]
async fn files_upload_uses_multipart_fixed_origin_and_exact_bytes() {
    let server = Server::start(vec![Reply::json(
        201,
        json!({"id":ID,"state":"pending","upload":null,"file":metadata("pending")}),
    )])
    .await;
    let result = within(
        &server.origin,
        execute(
            &server.client(),
            &payload(),
            prepare(json!({"action":"upload",
        "filename":"参考.png","purpose":"reference","data":"AQID"})),
        ),
    )
    .await
    .unwrap();
    assert_eq!(result["file"]["state"], "pending");
    let calls = server.calls();
    assert_eq!(calls.len(), 1);
    let call = &calls[0];
    assert_eq!((&*call.method, &*call.path), ("POST", "/v1/files"));
    assert_eq!(
        call.headers["authorization"],
        "Bearer luma-api-synthetic-only"
    );
    for name in ["cookie", "origin", "x-secret-grant", "x-management-token"] {
        assert!(!call.headers.contains_key(name));
    }
    let mut request = Request::post("/")
        .body(Body::from(call.raw_body.clone()))
        .unwrap();
    *request.headers_mut() = call.headers.clone();
    let mut form = Multipart::from_request(request, &()).await.unwrap();
    let purpose = form.next_field().await.unwrap().unwrap();
    assert_eq!(purpose.name(), Some("purpose"));
    assert_eq!(purpose.text().await.unwrap(), "reference");
    let file = form.next_field().await.unwrap().unwrap();
    assert_eq!(file.name(), Some("file"));
    assert_eq!(file.file_name(), Some("参考.png"));
    assert_eq!(file.bytes().await.unwrap().as_ref(), &[1, 2, 3]);
    assert!(form.next_field().await.unwrap().is_none());
    server.stop().await;
}

#[tokio::test]
async fn files_list_encodes_cursor_and_only_returns_whitelisted_metadata() {
    let mut file = metadata("ready");
    file["filename"] = Value::Null;
    file["url"] = json!("https://storage.example.test/?secret=private");
    file["failure_reason"] = json!("private");
    let server = Server::start(vec![Reply::json(
        200,
        json!({"data":[file],"has_more":true,"next_cursor":"next+/="}),
    )])
    .await;
    let result = within(
        &server.origin,
        execute(
            &server.client(),
            &payload(),
            Prepared::List(Some("cursor+/=&id=other".into())),
        ),
    )
    .await
    .unwrap();
    assert_eq!(result["next_cursor"], "next+/=");
    assert!(result["data"][0]["filename"].is_null());
    assert!(!result.to_string().contains("private"));
    let calls = server.calls();
    let url = url::Url::parse(&format!("{}{}", server.origin, calls[0].path)).unwrap();
    assert_eq!(
        url.query_pairs().collect::<Vec<_>>(),
        vec![
            ("limit".into(), "25".into()),
            ("cursor".into(), "cursor+/=&id=other".into())
        ]
    );
    server.stop().await;
}

#[tokio::test]
async fn files_rejects_mismatched_metadata_and_presigned_uploads() {
    let mut wrong = metadata("ready");
    wrong["id"] = json!("d290f1ee-6c54-4b01-90e6-d701748f0852");
    for (body, operation) in [
        (wrong, json!({"action":"get","id":ID})),
        (
            json!({"data":[metadata("ready"),metadata("ready")],"has_more":false}),
            json!({"action":"list"}),
        ),
        (
            json!({"data":[metadata("ready")],"has_more":true}),
            json!({"action":"list"}),
        ),
        (
            json!({"id":ID,"state":"pending","upload":{"url":"https://storage.test"},"file":metadata("pending")}),
            json!({"action":"upload","filename":"a.png","purpose":"input","data":"AQID"}),
        ),
    ] {
        let status = if operation["action"] == "upload" {
            201
        } else {
            200
        };
        let server = Server::start(vec![Reply::json(status, body)]).await;
        assert!(within(
            &server.origin,
            execute(&server.client(), &payload(), prepare(operation))
        )
        .await
        .is_err());
        assert_eq!(server.calls().len(), 1);
        server.stop().await;
    }
}

#[tokio::test]
async fn files_http_errors_html_and_decoded_limits_never_retry() {
    for (status, content_type, bytes) in [
        (302, "text/html", b"private".to_vec()),
        (429, "application/json", b"private".to_vec()),
        (200, "text/html", b"private".to_vec()),
        (200, "application/json", vec![b' '; 256 * 1024 + 1]),
    ] {
        let server = Server::start(vec![Reply {
            status,
            content_type,
            bytes,
            hold: false,
        }])
        .await;
        let error = within(
            &server.origin,
            execute(&server.client(), &payload(), Prepared::Get(ID.into())),
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("private"));
        assert_eq!(server.calls().len(), 1);
        server.stop().await;
    }
}

#[tokio::test]
async fn files_cancellation_drops_owned_request_without_resubmission() {
    let server = Server::start(vec![Reply {
        status: 200,
        content_type: "application/json",
        bytes: vec![],
        hold: true,
    }])
    .await;
    within(&server.origin, async {
        let client = server.client();
        let material = payload();
        let work = execute(&client, &material, Prepared::Get(ID.into()));
        tokio::pin!(work);
        tokio::select! { _ = &mut work => panic!("body should remain pending"),
        _ = tokio::time::sleep(Duration::from_millis(100)) => {} }
    })
    .await;
    server.wait_closed().await;
    assert_eq!(server.calls().len(), 1);
    server.stop().await;
}
