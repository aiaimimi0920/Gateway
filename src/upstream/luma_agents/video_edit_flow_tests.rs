//! Video edits use the original public/probe lifecycle and never become an untyped video task.
use super::{pipeline_fixture::*, probe_tests::options, test_support::*};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

fn parameters() -> Value {
    json!({"source":{"generation_id":"00000000-0000-0000-0000-000000000001"},
        "video":{"resolution":"720p","edit":{"strength":"flex_2","controls":{"pose":{"enabled":true,"strength":"coarse"}}}}})
}
fn task(state: &str) -> Value {
    let mut value = job(state);
    value["type"] = json!("video_edit");
    value["model"] = json!("ray-3.2");
    value["usage"] = json!({"credits_charged":0.75});
    if state == "completed" {
        value["output"][0]["type"] = json!("video");
    }
    if state == "failed" {
        value["failure_code"] = json!("content_moderated");
    }
    value
}
fn wire(server: &Server, success: bool) {
    let calls = server.calls();
    assert_eq!(calls.len(), if success { 3 } else { 1 });
    assert_eq!(calls.iter().filter(|c| c.method == "POST").count(), 1);
    assert_eq!(calls[0].path, "/v1/generations");
    assert_eq!(calls[0].body["type"], "video_edit");
    assert_eq!(calls[0].body["model"], "ray-3.2");
    assert_eq!(calls[0].body["source"], parameters()["source"]);
    assert_eq!(calls[0].body["video"], parameters()["video"]);
    for field in ["aspect_ratio", "duration", "image_ref"] {
        assert!(calls[0].body.get(field).is_none());
    }
    assert_eq!(calls[0].headers["authorization"], format!("Bearer {KEY}"));
    if success {
        assert_eq!(calls[1].path, format!("/v1/generations/{ID}"));
        assert_eq!(calls[1].method, "GET");
        assert_eq!(calls[1].headers["authorization"], format!("Bearer {KEY}"));
        assert!(!calls[2].headers.contains_key("authorization"));
    }
    for call in calls {
        assert!(!call.headers.contains_key("cookie"));
    }
}
async fn server(success: bool) -> Server {
    Server::start(if success {
        vec![
            Reply::json(201, task("queued")),
            Reply::json(200, task("completed")),
            Reply::mp4(),
        ]
    } else {
        vec![Reply::json(201, task("failed"))]
    })
    .await
}
fn facts(row: &crate::db::GatewayRequestAuditView, success: bool) {
    assert_eq!(row.response_id, ID);
    assert_eq!(row.status, if success { "completed" } else { "failed" });
    let facts = &row.route_trace.as_ref().unwrap()["lumaAgentsGeneration"];
    assert_eq!(facts["model"], "ray-3.2");
    assert_eq!(facts["type"], "video_edit");
    assert_eq!(facts["usage"]["credits_charged"], 0.75);
    assert!(row.total_tokens.is_none());
    let audit = serde_json::to_string(row).unwrap();
    assert!(!audit.contains(KEY) && !audit.contains("signature=private"));
}

#[tokio::test]
async fn luma_video_edit_public_success_and_failure_use_exact_account_and_original_billing() {
    Box::pin(async {
        for success in [true, false] {
            let server = server(success).await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            within(&server.origin, async {
                let mut input = body(VIDEO);
                input["type"] = json!("video_edit");
                input
                    .as_object_mut()
                    .unwrap()
                    .extend(parameters().as_object().unwrap().clone());
                let response = fixture.open("videos/generations", input).await;
                assert_eq!(
                    response.status(),
                    if success {
                        StatusCode::OK
                    } else {
                        StatusCode::BAD_GATEWAY
                    }
                );
                let text = drain(response.into_body()).await;
                assert!(!text.contains(KEY));
                if !success {
                    assert!(!text.contains(ID) && !text.contains("credits_charged"));
                }
                let row = fixture.audit().await;
                identity(&row, VIDEO, "ray-3.2");
                facts(&row, success);
                assert_eq!(
                    fixture.balance().await.remaining_messages,
                    Some(if success { 1 } else { 2 })
                );
                wire(&server, success);
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn luma_video_edit_management_success_and_failure_keep_audit_without_local_key_charge() {
    Box::pin(async {
        for success in [true, false] {
            let server = server(success).await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            let grant = Box::pin(support::grant_console_secret_access(&fixture.state)).await;
            within(
                &server.origin,
                Box::pin(async {
                    let mut input = options("video_edit", VIDEO);
                    input["media"]["parameters"] = parameters();
                    let response = Box::pin(support::send_console_json(
                        &fixture.state,
                        Method::POST,
                        "/v1/internal/gateway/console/providers/luma-pipeline/probe",
                        input,
                        Some(&grant),
                        None,
                    ))
                    .await;
                    assert_eq!(response.status(), StatusCode::OK);
                    let result = support::parse_json(response).await;
                    let report = &result["result"]["results"][0];
                    assert_eq!(report["credentialId"], "exact");
                    assert_eq!(
                        report["status"],
                        if success { "passed" } else { "failed" },
                        "{result}"
                    );
                    let row = fixture.audit().await;
                    facts(&row, success);
                    let trace = row.route_trace.as_ref().unwrap();
                    assert_eq!(trace["realCredentialRef"], "exact");
                    assert_eq!(
                        trace["consoleMediaProbe"],
                        json!({"operation":"video_edit","chargeConfirmed":true})
                    );
                    assert!(row.access_key_id.is_none());
                    assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                    wire(&server, success);
                }),
            )
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn luma_video_edit_public_invalid_parameters_never_submit_or_bill() {
    Box::pin(async {
        let server = Server::start(vec![]).await;
        let fixture = Box::pin(Fixture::new(&server, 5)).await;
        within(&server.origin, async {
            for change in [
                json!({"source":null}),
                json!({"source":{"url":"https://media.example.test/x"}}),
                json!({"aspect_ratio":"16:9"}),
                json!({"video":{"edit":{"auto_controls":true,"controls":{}}}}),
                json!({"video":{"hdr":true,"resolution":"360p"}}),
                json!({"model":IMAGE}),
            ] {
                let mut input = body(VIDEO);
                input["type"] = json!("video_edit");
                input
                    .as_object_mut()
                    .unwrap()
                    .extend(parameters().as_object().unwrap().clone());
                input
                    .as_object_mut()
                    .unwrap()
                    .extend(change.as_object().unwrap().clone());
                assert_eq!(
                    fixture.open("videos/generations", input).await.status(),
                    StatusCode::BAD_REQUEST
                );
                assert!(server.calls().is_empty());
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
            }
        })
        .await;
        fixture.finish().await;
        server.stop().await;
    })
    .await;
}

#[tokio::test]
async fn luma_video_edit_wrong_generation_kind_is_not_downloaded_retried_or_audited_as_success() {
    let mut wrong = task("completed");
    wrong["type"] = json!("video_reframe");
    let server = Server::start(vec![Reply::json(201, wrong)]).await;
    let mut input = body("ray-3.2");
    input["type"] = json!("video_edit");
    input
        .as_object_mut()
        .unwrap()
        .extend(parameters().as_object().unwrap().clone());
    let canonical = crate::protocol::gemini_canvas::normalize_video_generations(input).unwrap();
    let error = within(
        &server.origin,
        super::execute(
            &server.client(),
            std::time::Duration::from_secs(5),
            &payload(),
            &canonical,
            "ray-3.2",
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("luma_agents_generation"));
    assert!(error.luma_generation_failure.is_none());
    assert!(!error.retryable && !error.should_fallback());
    assert_eq!(server.calls().len(), 1);
    server.stop().await;
}
