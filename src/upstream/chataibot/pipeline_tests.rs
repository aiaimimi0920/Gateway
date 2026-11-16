//! Actual public generation/JSON and multipart edits, exact identity and SQLite settlement.
use super::{pipeline_fixture::*, test_support::*};
use axum::http::StatusCode;
use base64::Engine;
use serde_json::{json, Value};

fn wire(server: &Server, path: &str) {
    let calls = server.calls();
    assert_eq!(calls.len(), 3);
    assert_eq!((&*calls[0].method, &*calls[0].path), ("POST", path));
    assert_eq!(
        (&*calls[1].method, &*calls[1].path),
        ("GET", "/api/jobs/synthetic-job")
    );
    for call in &calls[..2] {
        assert_eq!(call.headers["cookie"], format!("token={TOKEN}"));
        assert_eq!(call.headers["user-agent"], "synthetic-account-ua");
        assert_eq!(call.headers["x-supports-async-image-jobs"], "true");
        assert!(!call.headers.contains_key("authorization"));
    }
    for name in [
        "authorization",
        "cookie",
        "origin",
        "referer",
        "x-api-key",
        "user-agent",
    ] {
        assert!(
            !calls[2].headers.contains_key(name),
            "asset must not inherit {name}"
        );
    }
}
fn replies() -> Vec<Reply> {
    vec![
        Reply::json(json!({"status":"pending","jobId":"synthetic-job"})),
        Reply::json(json!({"status":"completed","data":{"imageUrl":ASSET}})),
        Reply::png(),
    ]
}
#[tokio::test]
async fn chataibot_pipeline_generate_edit_and_merge_use_current_owner_and_settle_once() {
    Box::pin(async {
        for files in 0..=2 {
            let server = Server::start(replies()).await;
            let fixture = Box::pin(Fixture::new(5)).await;
            within(&server.origin, async {
                let mut input = body();
                if files > 0 {
                    input["images"] = json!((0..files)
                        .map(|_| json!({"mime_type":"image/png",
                    "base64":base64::engine::general_purpose::STANDARD.encode(Reply::png().bytes)}))
                        .collect::<Vec<_>>());
                }
                let endpoint = if files == 0 {
                    "images/generations"
                } else {
                    "images/edits"
                };
                let response = fixture.open(endpoint, input).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                let output: Value = serde_json::from_str(&text).unwrap();
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(output["data"][0]["b64_json"].as_str().unwrap())
                    .unwrap();
                assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 1);
                assert!(output.get("usage").is_none());
                let row = fixture.audit().await;
                assert_eq!(row.status, "completed");
                identity(&row);
                assert!(row.total_tokens.is_none());
                assert_eq!(fixture.balance().await.remaining_messages, Some(1));
                wire(
                    &server,
                    [
                        "/api/image/generate",
                        "/api/file/recognize",
                        "/api/file/merge",
                    ][files],
                );
                let calls = server.calls();
                if files == 0 {
                    let generated: Value = serde_json::from_slice(&calls[0].bytes).unwrap();
                    assert_eq!(generated["generationType"], "GOOGLE");
                    assert_eq!(generated["version"], "nano-banana-2");
                    assert_eq!(generated["text"], "合成图片测试");
                    assert!(
                        uuid::Uuid::parse_str(generated["requestId"].as_str().unwrap()).is_ok()
                    );
                } else {
                    assert!(calls[0].headers["content-type"]
                        .to_str()
                        .unwrap()
                        .starts_with("multipart/form-data; boundary="));
                    assert!(calls[0]
                        .bytes
                        .windows(Reply::png().bytes.len())
                        .any(|w| w == Reply::png().bytes));
                    assert!(
                        String::from_utf8_lossy(&calls[0].bytes).contains(if files == 1 {
                            "edit_google_nano_banana_2"
                        } else {
                            "merge_google_nano_banana_2"
                        })
                    );
                }
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn chataibot_pipeline_public_multipart_upload_is_not_lost_in_projection() {
    Box::pin(async {
        let server = Server::start(replies()).await; let fixture = Box::pin(Fixture::new(5)).await;
        within(&server.origin, async {
            let boundary = "chataibot-contract"; let png = Reply::png().bytes;
            let mut raw = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n{MODEL}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"prompt\"\r\n\r\n合成图片测试\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"input.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();
            raw.extend_from_slice(&png); raw.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
            let response = fixture.open_raw("images/edits", &format!("multipart/form-data; boundary={boundary}"), raw).await;
            let status = response.status(); let text = drain(response.into_body()).await;
            assert_eq!(status, StatusCode::OK, "{text}");
            assert_eq!(serde_json::from_str::<Value>(&text).unwrap()["data"][0]["url"], ASSET);
            let row = fixture.audit().await; identity(&row); assert_eq!(row.status, "completed");
            assert_eq!(fixture.balance().await.remaining_messages, Some(1)); wire(&server, "/api/file/recognize");
        }).await;
        fixture.finish().await; server.stop().await;
    }).await;
}
