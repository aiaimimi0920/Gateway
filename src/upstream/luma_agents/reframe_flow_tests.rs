//! Reframe uses the normal public/probe owners, with one job and isolated supplier facts.
use super::{pipeline_fixture::*, probe_tests::options, test_support::*};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

fn parameters() -> Value {
    json!({"source":{"generation_id":"00000000-0000-0000-0000-000000000001"},
        "aspect_ratio":"21:9", "video":{"duration":"10s","resolution":"720p"}})
}
fn task(state: &str) -> Value {
    let mut value = job(state);
    value["type"] = json!("video_reframe");
    value["model"] = json!("ray-3.2");
    value["usage"] = json!({"credits_charged":0.5});
    if state == "completed" {
        value["output"][0]["type"] = json!("video");
    }
    if state == "failed" {
        value["failure_code"] = json!("content_moderated");
    }
    value
}
fn wire(server: &Server, count: usize) {
    let calls = server.calls();
    assert_eq!(calls.len(), count);
    assert_eq!(calls.iter().filter(|call| call.method == "POST").count(), 1);
    assert_eq!(calls[0].path, "/v1/generations");
    assert_eq!(calls[0].body["type"], "video_reframe");
    assert_eq!(calls[0].body["model"], "ray-3.2");
    for field in ["source", "aspect_ratio", "video"] {
        assert_eq!(calls[0].body[field], parameters()[field]);
    }
    assert_eq!(calls[0].headers["authorization"], format!("Bearer {KEY}"));
    if count > 1 {
        assert_eq!(calls[1].method, "GET");
        assert_eq!(calls[1].path, format!("/v1/generations/{ID}"));
        assert_eq!(calls[1].headers["authorization"], format!("Bearer {KEY}"));
        assert!(!calls[2].headers.contains_key("authorization"));
    }
    for call in calls {
        assert!(!call.headers.contains_key("cookie"));
    }
}

#[tokio::test]
async fn luma_reframe_public_success_and_failure_keep_task_facts_and_existing_settlement() {
    Box::pin(async {
        for state in ["completed", "failed"] {
            let replies = if state == "completed" {
                vec![
                    Reply::json(201, task("queued")),
                    Reply::json(200, task(state)),
                    Reply::mp4(),
                ]
            } else {
                vec![Reply::json(201, task(state))]
            };
            let server = Server::start(replies).await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            within(&server.origin, async {
                let mut input = body(VIDEO);
                input["type"] = json!("video_reframe");
                input
                    .as_object_mut()
                    .unwrap()
                    .extend(parameters().as_object().unwrap().clone());
                let response = fixture.open("videos/generations", input).await;
                assert_eq!(
                    response.status(),
                    if state == "completed" {
                        StatusCode::OK
                    } else {
                        StatusCode::BAD_GATEWAY
                    }
                );
                let text = drain(response.into_body()).await;
                assert!(!text.contains(KEY));
                let row = fixture.audit().await;
                identity(&row, VIDEO, "ray-3.2");
                assert_eq!(row.status, state);
                assert_eq!(row.response_id, ID);
                let facts = &row.route_trace.as_ref().unwrap()["lumaAgentsGeneration"];
                assert_eq!(facts["type"], "video_reframe");
                assert_eq!(facts["usage"]["credits_charged"], 0.5);
                assert!(row.total_tokens.is_none());
                assert_eq!(
                    fixture.balance().await.remaining_messages,
                    Some(if state == "completed" { 1 } else { 2 })
                );
                wire(&server, if state == "completed" { 3 } else { 1 });
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn luma_reframe_management_probe_keeps_explicit_operation_account_and_no_local_charge() {
    Box::pin(async {
        let server = Server::start(vec![
            Reply::json(201, task("queued")),
            Reply::json(200, task("completed")),
            Reply::mp4(),
        ])
        .await;
        let fixture = Box::pin(Fixture::new(&server, 5)).await;
        let grant = Box::pin(support::grant_console_secret_access(&fixture.state)).await;
        within(
            &server.origin,
            Box::pin(async {
                let mut input = options("video_reframe", VIDEO);
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
                assert_eq!(
                    result["result"]["results"][0]["status"], "passed",
                    "{result}"
                );
                assert_eq!(result["result"]["results"][0]["credentialId"], "exact");
                let row = fixture.audit().await;
                assert_eq!(row.response_id, ID);
                let trace = row.route_trace.as_ref().unwrap();
                assert_eq!(
                    trace["consoleMediaProbe"],
                    json!({"operation":"video_reframe","chargeConfirmed":true})
                );
                assert_eq!(trace["realCredentialRef"], "exact");
                assert_eq!(trace["lumaAgentsGeneration"]["type"], "video_reframe");
                assert!(row.access_key_id.is_none() && row.total_tokens.is_none());
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                wire(&server, 3);
            }),
        )
        .await;
        fixture.finish().await;
        server.stop().await;
    })
    .await;
}

#[tokio::test]
async fn luma_reframe_public_invalid_parameters_never_submit_or_bill() {
    Box::pin(async {
        let server = Server::start(vec![]).await;
        let fixture = Box::pin(Fixture::new(&server, 5)).await;
        within(&server.origin, async {
            for change in [
                json!({"source":null}),
                json!({"aspect_ratio":null}),
                json!({"aspect_ratio":"9:16","video":{"resolution":"1080p"}}),
                json!({"type":"video_edit"}),
            ] {
                let mut input = body(VIDEO);
                input["type"] = json!("video_reframe");
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
