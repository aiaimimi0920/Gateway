//! Public Accio inference wire evidence, not merely planner or login coverage.
#![cfg(feature = "line-accio-web-reverse-api")]
#[path = "accio_relay_contract/fixture.rs"]
mod fixture;
#[path = "console_contract_support/mod.rs"]
mod support;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use futures::StreamExt;
use http_body_util::BodyExt;
use neuro_gateway::http::router::build_router;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn relay(
    server: &fixture::Upstream,
    endpoint: &str,
    stream: bool,
) -> (StatusCode, String, Option<String>) {
    let f = support::ConsoleStateFixture::new(server.document(&["good"]), false);
    let mut body = json!({"model":fixture::MODEL,"stream":stream,"token":"caller-forbidden","utdid":"caller-device"});
    if endpoint == "responses" {
        body["input"] = json!("Explicit user prompt");
    } else {
        body["messages"] = json!([{"role":"user","content":"Explicit user prompt"}]);
    }
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(90),
        build_router(f.state.clone()).oneshot(
            Request::post(format!("/v1/{endpoint}"))
                .header("authorization", "Bearer public-api-contract-secret")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ),
    )
    .await
    .expect("bounded public Accio request")
    .unwrap();
    let status = response.status();
    let mut stream = response.into_body().into_data_stream();
    let (body, error) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut bytes = Vec::new();
        let mut error = None;
        while let Some(item) = stream.next().await {
            match item {
                Ok(chunk) => bytes.extend_from_slice(&chunk),
                Err(failure) => {
                    error = Some(failure.to_string());
                    break;
                }
            }
            assert!(bytes.len() <= 65536);
        }
        (bytes, error)
    })
    .await
    .expect("bounded public Accio response body");
    (status, String::from_utf8(body).unwrap(), error)
}
async fn normal(endpoint: &str, stream: bool) {
    let server = fixture::Upstream::start(Some("good")).await;
    let (status, text, error) = relay(&server, endpoint, stream).await;
    assert!(error.is_none(), "{error:?}");
    assert_eq!(status, StatusCode::OK, "{text}");
    let calls = server.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1, "no hidden retry or discovery");
    assert_eq!(calls[0].0, "/api/adk/llm/generateContent");
    assert_eq!(calls[0].1["accept"], "text/event-stream");
    assert_eq!(calls[0].1["utdid"], "good-device");
    assert_eq!(calls[0].2["token"], "good-secret");
    assert_eq!(calls[0].2["utdid"], "good-device");
    assert_eq!(calls[0].2["model"], fixture::MODEL);
    assert_eq!(calls[0].2["incremental"], true);
    assert!(calls[0].2.to_string().contains("Explicit user prompt"));
    assert!(!calls[0].2.to_string().contains("caller-forbidden"));
    if stream {
        assert!(text.contains("OK 你好"), "{text}");
        assert!(
            text.contains(if endpoint == "responses" {
                "response.completed"
            } else {
                "[DONE]"
            }),
            "{text}"
        );
        assert!(
            text.contains("\"total_tokens\":9") || text.contains("\"total_tokens\": 9"),
            "{text}"
        );
    } else {
        let body: Value = serde_json::from_str(&text).unwrap();
        if endpoint == "responses" {
            assert_eq!(body["status"], "completed");
            assert_eq!(body["output"][0]["content"][0]["text"], "OK 你好");
        } else {
            assert_eq!(body["choices"][0]["message"]["content"], "OK 你好");
            assert_eq!(body["choices"][0]["finish_reason"], "stop");
        }
        assert_eq!(
            body["usage"]["total_tokens"], 9,
            "reported total must not be recomputed"
        );
        if endpoint == "chat/completions" {
            assert_eq!(body["usage"]["prompt_tokens_details"]["cached_tokens"], 4);
        }
    }
    server.finish().await;
}
#[test]
fn accio_chat_buffered_has_native_wire_and_usage() {
    fixture::on_runtime(|| normal("chat/completions", false));
}
#[test]
fn accio_chat_stream_has_native_wire_and_usage() {
    fixture::on_runtime(|| normal("chat/completions", true));
}
#[test]
fn accio_responses_buffered_has_native_wire_and_usage() {
    fixture::on_runtime(|| normal("responses", false));
}
#[test]
fn accio_responses_stream_has_native_wire_and_usage() {
    fixture::on_runtime(|| normal("responses", true));
}

#[test]
fn accio_buffered_rejects_empty_unknown_truncated_and_business_errors() {
    fixture::on_runtime(|| async {
        for case in [
            "empty",
            "garbage",
            "truncated",
            "mid-error",
            "split-error",
            "tail",
            "usage-only",
            "done-only",
            "orphan-tool",
        ] {
            let server = fixture::Upstream::start(Some(case)).await;
            let (status, body, error) = relay(&server, "chat/completions", false).await;
            assert!(error.is_none(), "{case}: {error:?}");
            assert!(!status.is_success(), "{case}: {body}");
            assert_eq!(server.calls.lock().unwrap().len(), 1, "{case}");
            assert!(!body.contains("synthetic-private-upstream"), "{body}");
            server.finish().await;
        }
    });
}

#[test]
fn accio_stream_does_not_hide_midstream_or_split_business_errors() {
    fixture::on_runtime(|| async {
        for case in [
            "mid-error",
            "split-error",
            "truncated",
            "tail",
            "usage-only",
            "done-only",
            "orphan-tool",
        ] {
            let server = fixture::Upstream::start(Some(case)).await;
            let (status, body, error) = relay(&server, "chat/completions", true).await;
            assert!(!body.contains("[DONE]"), "{case}: {body}");
            assert!(
                !body.contains("\"finish_reason\":\"stop\""),
                "{case}: {body}"
            );
            assert!(
                error.is_some() || !status.is_success(),
                "{case}: missing explicit stream/HTTP failure: {body}"
            );
            assert!(!format!("{error:?}").contains("synthetic-private-upstream"));
            assert!(!body.contains("synthetic-private-upstream"), "{body}");
            assert_eq!(server.calls.lock().unwrap().len(), 1);
            server.finish().await;
        }
    });
}

#[test]
fn accio_stream_tool_arguments_are_not_replayed_at_finish() {
    fixture::on_runtime(|| async {
        let server = fixture::Upstream::start(Some("tool")).await;
        let (status, body, error) = relay(&server, "chat/completions", true).await;
        assert!(error.is_none(), "{error:?}");
        assert_eq!(status, StatusCode::OK, "{body}");
        let mut args = String::new();
        let mut identities = 0;
        for data in body.lines().filter_map(|line| line.strip_prefix("data: ")) {
            if let Ok(value) = serde_json::from_str::<Value>(data) {
                if let Some(call) = value.pointer("/choices/0/delta/tool_calls/0") {
                    assert_eq!(call["index"], 0);
                    identities += usize::from(call.get("id").is_some());
                    args.push_str(call["function"]["arguments"].as_str().unwrap());
                }
            }
        }
        assert_eq!(args, "{\"city\":\"杭州\"}", "{body}");
        assert_eq!(identities, 1);
        assert_eq!(body.matches("[DONE]").count(), 1);
        server.finish().await;
    });
}

#[test]
fn accio_consumer_drop_releases_the_inflight_native_body() {
    fixture::on_runtime(|| async {
        use futures::StreamExt;
        use std::sync::atomic::Ordering;
        use std::time::Duration;
        let server = fixture::Upstream::start(Some("hang")).await;
        let f = support::ConsoleStateFixture::new(server.document(&["good"]), false);
        let response=build_router(f.state.clone()).oneshot(Request::post("/v1/chat/completions")
            .header("authorization","Bearer public-api-contract-secret").header("content-type","application/json")
            .body(Body::from(json!({"model":fixture::MODEL,"stream":true,"messages":[{"role":"user","content":"cancel fixture"}]}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body().into_data_stream();
        let first = tokio::time::timeout(Duration::from_millis(500), body.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(!String::from_utf8_lossy(&first).contains("[DONE]"));
        drop(body);
        tokio::time::timeout(Duration::from_millis(500),async {
            while !server.dropped.load(Ordering::SeqCst) {tokio::time::sleep(Duration::from_millis(5)).await;}
        }).await.expect("dropping public body must release native upstream, not wait for its 1s transport timeout");
        assert_eq!(server.calls.lock().unwrap().len(), 1);
        server.finish().await;
    });
}

#[test]
fn accio_multiple_complete_tool_calls_keep_distinct_stream_indexes() {
    fixture::on_runtime(|| async {
        let server = fixture::Upstream::start(Some("two-tools")).await;
        let (_, body, error) = relay(&server, "chat/completions", true).await;
        assert!(error.is_none(), "{error:?}");
        let calls: Vec<Value> = body
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .filter_map(|data| serde_json::from_str::<Value>(data).ok())
            .filter_map(|value| value.pointer("/choices/0/delta/tool_calls/0").cloned())
            .collect();
        assert_eq!(calls.len(), 2, "{body}");
        assert_eq!(calls[0]["index"], 0);
        assert_eq!(calls[1]["index"], 1, "{body}");
        assert_eq!(calls[0]["id"], "call-1");
        assert_eq!(calls[1]["id"], "call-2");
        server.finish().await;
    });
}
