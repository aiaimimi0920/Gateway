//! Supplier charges on failed tasks are audit facts, not successful local requests.
use super::{pipeline_fixture::*, probe_tests::options, test_support::*};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

pub(super) fn failed(kind: &str, model: &str, credits: Value) -> Value {
    let mut value = job("failed");
    value["type"] = json!(kind);
    value["model"] = json!(model);
    value["failure_code"] = json!("content_moderated");
    value["failure_reason"] = json!(format!("supplier-private {KEY}"));
    value["usage"] = if credits.is_null() {
        Value::Null
    } else {
        json!({"credits_charged":credits})
    };
    value
}

fn assert_audit(
    row: &crate::db::GatewayRequestAuditView,
    kind: &str,
    model: &str,
    credits: &Value,
) {
    assert_eq!(row.status, "failed");
    assert_eq!(row.response_id, ID);
    assert!(row.total_tokens.is_none());
    assert_eq!(
        row.route_trace.as_ref().unwrap()["lumaAgentsGeneration"],
        json!({
            "id":ID,"model":model,"type":kind,"state":"failed","usage":{"credits_charged":credits}
        })
    );
    let text = serde_json::to_string(row).unwrap();
    assert!(
        !text.contains(KEY)
            && !text.contains("supplier-private")
            && !text.contains("signature=private")
    );
}

fn wire(server: &Server, count: usize) {
    let calls = server.calls();
    assert_eq!(calls.len(), count);
    assert_eq!(calls.iter().filter(|call| call.method == "POST").count(), 1);
    for call in &calls {
        assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
        assert!(!call.headers.contains_key("cookie"));
    }
    for call in &calls[1..] {
        assert_eq!(call.method, "GET");
        assert_eq!(call.path, format!("/v1/generations/{ID}"));
    }
}

#[tokio::test]
async fn luma_failure_audit_public_failure_retains_charges_and_refunds_local_key() {
    Box::pin(async {
        for (endpoint, kind, model, resolved, credits) in [
            ("images/generations", "image", IMAGE, "uni-1", Value::Null),
            ("images/edits", "image_edit", IMAGE, "uni-1", json!(0)),
            ("videos/generations", "video", VIDEO, "ray-3.2", json!(1.25)),
        ] {
            let server = Server::start(vec![Reply::json(
                201,
                failed(kind, resolved, credits.clone()),
            )])
            .await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            within(&server.origin, async {
                let mut input = body(model);
                if kind == "image_edit" {
                    input["source"] = json!({"generation_id":ID});
                }
                let response = fixture.open(endpoint, input).await;
                assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
                let text = drain(response.into_body()).await;
                let output: Value = serde_json::from_str(&text).unwrap();
                assert_eq!(output["error"]["code"], "luma_agents_content_moderated");
                assert!(
                    !text.contains(ID) && !text.contains("credits_charged") && !text.contains(KEY)
                );
                let row = fixture.audit().await;
                identity(&row, model, resolved);
                assert_audit(&row, kind, resolved, &credits);
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                wire(&server, 1);
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn luma_failure_audit_management_preserves_failed_status_and_late_charge() {
    Box::pin(async {
        let mut pending = failed("image", "uni-1", Value::Null);
        pending["usage"] = json!({"credits_charged":null});
        let server = Server::start(vec![
            Reply::json(201, pending),
            Reply::json(200, failed("image", "uni-1", json!(0.5))),
        ])
        .await;
        let fixture = Box::pin(Fixture::new(&server, 5)).await;
        let grant = Box::pin(support::grant_console_secret_access(&fixture.state)).await;
        within(
            &server.origin,
            Box::pin(async {
                let response = Box::pin(support::send_console_json(
                    &fixture.state,
                    Method::POST,
                    "/v1/internal/gateway/console/providers/luma-pipeline/probe",
                    options("image", IMAGE),
                    Some(&grant),
                    None,
                ))
                .await;
                assert_eq!(response.status(), StatusCode::OK);
                let output = support::parse_json(response).await;
                assert_eq!(output["result"]["results"][0]["status"], "failed");
                let row = fixture.audit().await;
                assert_audit(&row, "image", "uni-1", &json!(0.5));
                assert_eq!(
                    row.route_trace.as_ref().unwrap()["realCredentialRef"],
                    "exact"
                );
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                wire(&server, 2);
            }),
        )
        .await;
        fixture.finish().await;
        server.stop().await;
    })
    .await;
}
