//! Normal public API / management router / SQLite proof, not a real supplier generation.
use super::{layering_fixture::*, pipeline_fixture::*, probe_tests::options, test_support::*};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

fn facts(row: &crate::db::GatewayRequestAuditView, success: bool) {
    assert_eq!(row.status, if success { "completed" } else { "failed" });
    assert_eq!(row.response_id, ID);
    let native = &row.route_trace.as_ref().unwrap()["lumaAgentsGeneration"];
    assert_eq!(native["type"], "layering");
    assert_eq!(native["model"], "uni-1");
    assert_eq!(native["usage"]["credits_charged"], 0.75);
    assert!(row.total_tokens.is_none());
    let encoded = serde_json::to_string(row).unwrap();
    assert!(!encoded.contains(KEY) && !encoded.contains("signature=private"));
}
#[tokio::test]
async fn luma_layering_public_empty_prompt_preserves_layers_and_original_success_failure_settlement(
) {
    Box::pin(async {
        for success in [true, false] {
            let server = Server::start(if success {
                replies(2)
            } else {
                vec![Reply::json(201, task("failed", 2))]
            })
            .await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            within(&server.origin, async {
                let response = fixture.open("images/generations", input()).await;
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
                if success {
                    let result: Value = serde_json::from_str(&text).unwrap();
                    assert_eq!(result["data"].as_array().unwrap().len(), 2);
                    assert_eq!(result["data"][0]["layer"]["index"], 0);
                    assert_eq!(result["data"][1]["layer"]["alpha_hint"], "none");
                } else {
                    assert!(!text.contains(ID) && !text.contains("credits_charged"));
                }
                let row = fixture.audit().await;
                identity(&row, IMAGE, "uni-1");
                facts(&row, success);
                assert_eq!(
                    fixture.balance().await.remaining_messages,
                    Some(if success { 1 } else { 2 })
                );
                let calls = server.calls();
                assert_eq!(calls.len(), if success { 4 } else { 1 });
                assert_eq!(calls.iter().filter(|c| c.method == "POST").count(), 1);
                assert_eq!(calls[0].headers["authorization"], format!("Bearer {KEY}"));
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn luma_layering_management_empty_guidance_uses_exact_account_grant_and_no_local_charge() {
    Box::pin(async {
        for success in [true, false] {
            let server = Server::start(if success {
                replies(2)
            } else {
                vec![Reply::json(201, task("failed", 2))]
            })
            .await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            let grant = Box::pin(support::grant_console_secret_access(&fixture.state)).await;
            within(
                &server.origin,
                Box::pin(async {
                    let mut request = options("layering", IMAGE);
                    request["prompt"] = json!("");
                    request["media"]["parameters"] =
                        json!({"source":input()["source"],"layering":{"resolution":"2k"}});
                    let response = Box::pin(support::send_console_json(
                        &fixture.state,
                        Method::POST,
                        "/v1/internal/gateway/console/providers/luma-pipeline/probe",
                        request,
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
                    assert!(row.access_key_id.is_none());
                    let trace = row.route_trace.as_ref().unwrap();
                    assert_eq!(trace["realCredentialRef"], "exact");
                    assert_eq!(
                        trace["consoleMediaProbe"],
                        json!({"operation":"layering","chargeConfirmed":true})
                    );
                    assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                    assert_eq!(server.calls().len(), if success { 4 } else { 1 });
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
async fn luma_layering_public_invalid_parameters_never_submit_or_consume_a_message() {
    Box::pin(async {
        let server = Server::start(vec![]).await;
        let fixture = Box::pin(Fixture::new(&server, 5)).await;
        within(&server.origin, async {
            for change in [
                json!({"source":null}),
                json!({"prompt":"x".repeat(501)}),
                json!({"prompt":null}),
                json!({"layering":{"resolution":"4k"}}),
                json!({"layering":{"count":2}}),
                json!({"video":{}}),
                json!({"image_ref":[]}),
                json!({"web_search":false}),
                json!({"model":VIDEO}),
            ] {
                let mut value = input();
                value
                    .as_object_mut()
                    .unwrap()
                    .extend(change.as_object().unwrap().clone());
                assert_eq!(
                    fixture.open("images/generations", value).await.status(),
                    StatusCode::BAD_REQUEST,
                    "{change}"
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
async fn luma_layering_public_bad_last_asset_returns_no_partial_data_and_refunds() {
    Box::pin(async {
        let mut sequence = replies(2);
        sequence[3].bytes = b"corrupt PNG".to_vec();
        let server = Server::start(sequence).await;
        let fixture = Box::pin(Fixture::new(&server, 5)).await;
        within(&server.origin, async {
            let response = fixture.open("images/generations", input()).await;
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
            let text = drain(response.into_body()).await;
            assert!(
                !text.contains("b64_json")
                    && !text.contains(KEY)
                    && !text.contains("signature=private")
            );
            let row = fixture.audit().await;
            assert_eq!(row.status, "failed");
            assert_eq!(fixture.balance().await.remaining_messages, Some(2));
            assert_eq!(server.calls().len(), 4);
            assert_eq!(
                server.calls().iter().filter(|c| c.method == "POST").count(),
                1
            );
        })
        .await;
        fixture.finish().await;
        server.stop().await;
    })
    .await;
}
