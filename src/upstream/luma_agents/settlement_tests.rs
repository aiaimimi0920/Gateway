//! Late supplier charges must reach the caller and SQLite audit without replaying media generation.
use super::{execute, pipeline_fixture::*, test_support::*};
use axum::http::StatusCode;
use serde_json::{json, Value};
use std::time::Duration;

pub(super) fn pending() -> Value {
    let mut value = job("completed");
    value["usage"] = json!({"credits_charged":null});
    value
}
pub(super) async fn run(server: &Server, duration: Duration) -> Value {
    within(
        &server.origin,
        execute(
            &server.client(),
            duration,
            &payload(),
            &request(json!({"prompt":"image","response_format":"b64_json"})),
            "uni-1",
        ),
    )
    .await
    .unwrap()
}
pub(super) fn verified_unknown(output: &Value) {
    assert!(!output["data"][0]["b64_json"].as_str().unwrap().is_empty());
    assert!(output["gateway_luma_generation"]["usage"]["credits_charged"].is_null());
}
pub(super) fn one_generation(server: &Server, count: usize) {
    let calls = server.calls();
    assert_eq!(calls.len(), count);
    assert_eq!(calls.iter().filter(|c| c.method == "POST").count(), 1);
    for call in &calls[2..] {
        assert_eq!(call.method, "GET");
        assert_eq!(call.path, format!("/v1/generations/{ID}"));
        assert_eq!(
            call.headers["authorization"],
            "Bearer luma-api-synthetic-only"
        );
        assert!(!call.headers.contains_key("cookie"));
    }
}

#[tokio::test]
async fn luma_agents_settlement_late_zero_and_nonzero_reach_public_response_and_audit_once() {
    Box::pin(async {
        for credits in [json!(0), json!(1.25)] {
            let mut settled = pending();
            settled["usage"]["credits_charged"] = credits.clone();
            // Settlement must not read a replacement asset, even if the returned URL changes.
            settled["output"][0]["url"] = json!("https://unrelated.invalid/never-download");
            let server = Server::start(vec![
                Reply::json(201, pending()),
                Reply::png(),
                Reply::json(200, pending()),
                Reply::json(200, settled),
            ])
            .await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            within(&server.origin, async {
                let response = fixture.open("images/generations", body(IMAGE)).await;
                assert_eq!(response.status(), StatusCode::OK);
                let text = drain(response.into_body()).await;
                let output: Value = serde_json::from_str(&text).unwrap();
                assert!(!output["data"][0]["b64_json"].as_str().unwrap().is_empty());
                assert_eq!(
                    output["gateway_luma_generation"]["usage"]["credits_charged"],
                    credits
                );
                assert!(!text.contains("never-download") && !text.contains(KEY));
                let row = fixture.audit().await;
                assert_eq!(row.status, "completed");
                identity(&row, IMAGE, "uni-1");
                assert_eq!(
                    row.route_trace.as_ref().unwrap()["lumaAgentsGeneration"]["usage"]
                        ["credits_charged"],
                    credits
                );
                assert_eq!(fixture.balance().await.remaining_messages, Some(1));
                assert!(row.total_tokens.is_none());
                let calls = server.calls();
                assert_eq!(calls.len(), 4);
                assert_eq!(calls.iter().filter(|c| c.method == "POST").count(), 1);
                assert!(!calls[1].headers.contains_key("authorization"));
                for call in &calls[2..] {
                    assert_eq!(call.path, format!("/v1/generations/{ID}"));
                    assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
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
async fn luma_agents_settlement_known_or_unavailable_usage_never_adds_reads() {
    for usage in [
        Value::Null,
        json!({"credits_charged":0}),
        json!({"credits_charged":0.25}),
    ] {
        let mut completed = job("completed");
        completed["usage"] = usage.clone();
        let server = Server::start(vec![Reply::json(201, completed), Reply::png()]).await;
        let output = run(&server, Duration::from_secs(5)).await;
        assert_eq!(
            output["gateway_luma_generation"]["usage"]["credits_charged"],
            usage["credits_charged"]
        );
        one_generation(&server, 2);
        server.stop().await;
    }
    let mut completed = job("completed");
    completed.as_object_mut().unwrap().remove("usage");
    let server = Server::start(vec![Reply::json(201, completed), Reply::png()]).await;
    verified_unknown(&run(&server, Duration::from_secs(5)).await);
    one_generation(&server, 2);
}

#[tokio::test]
async fn luma_agents_settlement_pending_is_bounded_and_unavailable_stops_polling() {
    for reporting_stops in [false, true] {
        let mut replies = vec![Reply::json(201, pending()), Reply::png()];
        if reporting_stops {
            replies.push(Reply::json(200, job("completed")));
        } else {
            replies.extend((0..3).map(|_| Reply::json(200, pending())));
        }
        let server = Server::start(replies).await;
        verified_unknown(&run(&server, Duration::from_secs(5)).await);
        one_generation(&server, if reporting_stops { 3 } else { 5 });
        server.stop().await;
    }
}

#[tokio::test]
async fn luma_agents_settlement_rejects_foreign_identity_state_and_malformed_charges() {
    for (field, value) in [
        ("id", json!("00000000-0000-0000-0000-000000000001")),
        ("id", json!("../private")),
        ("model", json!("uni-1-max")),
        ("type", json!("video")),
        ("state", json!("failed")),
        ("state", json!("processing")),
        ("failure_reason", json!("private reason")),
        ("usage", json!("private")),
        ("usage", json!({"credits_charged":-1})),
        ("usage", json!({"credits_charged":"1"})),
    ] {
        let mut invalid = pending();
        invalid["usage"]["credits_charged"] = json!(99);
        invalid[field] = value;
        let server = Server::start(vec![
            Reply::json(201, pending()),
            Reply::png(),
            Reply::json(200, invalid),
        ])
        .await;
        let output = run(&server, Duration::from_secs(5)).await;
        verified_unknown(&output);
        assert!(!output.to_string().contains("private"));
        one_generation(&server, 3);
        server.stop().await;
    }
}

#[tokio::test]
async fn luma_agents_settlement_transport_errors_keep_verified_media_without_retry() {
    let mut replies: Vec<_> = [301, 401, 403, 429, 500, 503]
        .into_iter()
        .map(|status| Reply::json(status, json!({"private":"luma-api-secret"})))
        .collect();
    replies.extend([
        Reply {
            content_type: "text/html",
            ..Reply::json(200, pending())
        },
        Reply {
            bytes: b"{truncated".to_vec(),
            ..Reply::json(200, pending())
        },
        Reply {
            bytes: vec![b' '; 65537],
            ..Reply::json(200, pending())
        },
    ]);
    for reply in replies {
        let server = Server::start(vec![Reply::json(201, pending()), Reply::png(), reply]).await;
        verified_unknown(&run(&server, Duration::from_secs(5)).await);
        one_generation(&server, 3);
        server.stop().await;
    }
}
