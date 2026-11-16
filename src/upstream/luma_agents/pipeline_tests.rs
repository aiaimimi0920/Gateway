//! Public media requests must retain exact identity, verified output and native supplier facts.
use super::{pipeline_fixture::*, test_support::*};
use axum::http::StatusCode;
use base64::Engine;
use serde_json::{json, Value};

fn job_for(state: &str, kind: &str, model: &str, credits: Value) -> Value {
    let mut value = job(state);
    value["type"] = json!(kind);
    value["model"] = json!(model);
    value["usage"]["credits_charged"] = credits;
    if kind == "video" && state == "completed" {
        value["output"][0]["type"] = json!("video");
    }
    value
}
fn wire(server: &Server, kind: &str, model: &str) {
    let calls = server.calls();
    assert_eq!(
        calls.len(),
        3,
        "one submit, one original-ID poll, one asset"
    );
    assert_eq!(
        (&*calls[0].method, &*calls[0].path),
        ("POST", "/v1/generations")
    );
    assert_eq!(calls[0].body["type"], kind);
    assert_eq!(calls[0].body["model"], model);
    assert_eq!(calls[0].body["prompt"], "合成媒体测试");
    assert_eq!(calls[1].method, "GET");
    assert_eq!(calls[1].path, format!("/v1/generations/{ID}"));
    for call in &calls[..2] {
        assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
        for name in ["cookie", "origin", "x-api-key", "x-goog-api-key"] {
            assert!(!call.headers.contains_key(name));
        }
    }
    assert_eq!(calls[2].method, "GET");
    assert_eq!(calls[2].path, "/output.png?signature=private");
    for name in ["authorization", "cookie", "origin", "x-api-key"] {
        assert!(!calls[2].headers.contains_key(name));
    }
}
#[tokio::test]
async fn luma_agents_pipeline_images_edits_and_video_settle_once_and_persist_native_facts() {
    Box::pin(async {
        for (endpoint, kind, model, resolved, credits) in [
            ("images/generations", "image", IMAGE, "uni-1", Value::Null),
            ("images/edits", "image_edit", IMAGE, "uni-1", json!(0)),
            ("videos/generations", "video", VIDEO, "ray-3.2", json!(0.25)),
        ] {
            let server = Server::start(vec![
                Reply::json(201, job_for("queued", kind, resolved, Value::Null)),
                Reply::json(200, job_for("completed", kind, resolved, credits.clone())),
                if kind == "video" {
                    Reply::mp4()
                } else {
                    Reply::png()
                },
            ])
            .await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            within(&server.origin, async {
                let mut input = body(model);
                if kind == "image_edit" {
                    input["images"] = json!([{"mime_type":"image/png","base64":base64::engine::general_purpose::STANDARD.encode(Reply::png().bytes)}]);
                }
                let response = fixture.open(endpoint, input).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                let output: Value = serde_json::from_str(&text).unwrap();
                assert_eq!(
                    output["gateway_luma_generation"]["usage"]["credits_charged"],
                    credits
                );
                assert!(!text.contains(KEY) && !text.contains("private"), "{text}");
                let row = fixture.audit().await;
                assert_eq!(row.status, "completed");
                identity(&row, model, resolved);
                assert_eq!(fixture.balance().await.remaining_messages, Some(1));
                assert_eq!(
                    (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                    (None, None, None)
                );
                wire(&server, kind, resolved);
                assert_eq!(row.response_id, ID);
                let facts = &row.route_trace.as_ref().unwrap()["lumaAgentsGeneration"];
                assert_eq!(facts["id"], ID);
                assert_eq!(facts["type"], kind);
                assert_eq!(facts["model"], resolved);
                assert_eq!(facts["usage"]["credits_charged"], credits);
                let audit = serde_json::to_string(&row).unwrap();
                assert!(
                    !audit.contains(KEY) && !audit.contains("signature=private"),
                    "{audit}"
                );
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn luma_agents_pipeline_public_upload_projection_reaches_edit_source() {
    Box::pin(async {
        let server = Server::start(vec![Reply::json(201,job_for("queued","image_edit","uni-1",Value::Null)),
            Reply::json(200,job_for("completed","image_edit","uni-1",Value::Null)),Reply::png()]).await;
        let fixture=Box::pin(Fixture::new(&server,5)).await;
        within(&server.origin,async {
            let boundary="luma-contract-boundary";
            let png=Reply::png().bytes;
            let mut raw=format!("--{boundary}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n{IMAGE}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"prompt\"\r\n\r\n合成媒体测试\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"response_format\"\r\n\r\nb64_json\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"input.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();
            raw.extend_from_slice(&png);
            raw.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
            let response=fixture.open_raw("images/edits",&format!("multipart/form-data; boundary={boundary}"),raw).await;
            let status=response.status();
            let text=drain(response.into_body()).await;
            assert_eq!(status,StatusCode::OK,"{text}");
            assert_eq!(fixture.audit().await.status,"completed");
            assert_eq!(fixture.balance().await.remaining_messages,Some(1));
            wire(&server,"image_edit","uni-1");
            assert_eq!(server.calls()[0].body["source"],json!({"data":base64::engine::general_purpose::STANDARD.encode(png),"media_type":"image/png"}));
        }).await;
        fixture.finish().await;
        server.stop().await;
    }).await;
}
#[tokio::test]
async fn luma_agents_pipeline_source_refs_reach_image_edit_without_upload_normalizer_rejection() {
    Box::pin(async {
        for source in [
            json!({"url":"https://images.example.test/input.png"}),
            json!({"generation_id":ID}),
            json!({"file_id":ID}),
            json!({"data":base64::engine::general_purpose::STANDARD.encode(Reply::png().bytes),"media_type":"image/png"}),
        ] {
            let server = Server::start(vec![
                Reply::json(201, job_for("queued", "image_edit", "uni-1", Value::Null)),
                Reply::json(
                    200,
                    job_for("completed", "image_edit", "uni-1", Value::Null),
                ),
                Reply::png(),
            ])
            .await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            within(&server.origin, async {
                let mut input = body(IMAGE);
                input["source"] = source.clone();
                let response = fixture.open("images/edits", input).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                assert_eq!(fixture.audit().await.status, "completed");
                assert_eq!(fixture.balance().await.remaining_messages, Some(1));
                wire(&server, "image_edit", "uni-1");
                assert_eq!(server.calls()[0].body["source"], source);
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
