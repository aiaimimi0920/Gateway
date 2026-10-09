//! Seven client wires must reach the requested discovered upstream without fallback.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use neuro_gateway::{
    http::router::build_router, provider_discovery::binding, routing::config::RouteConfigYaml,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

fn inputs() -> Vec<(&'static str, Value)> {
    vec![
        (
            "/v1/chat/completions",
            json!({"model":"fixture","messages":[{"role":"user","content":"西片的男朋友是谁？"}],"max_tokens":32}),
        ),
        (
            "/v1/responses",
            json!({"model":"fixture","input":"西片的男朋友是谁？","max_output_tokens":32}),
        ),
        (
            "/v1/messages",
            json!({"model":"fixture","messages":[{"role":"user","content":"西片的男朋友是谁？"}],"max_tokens":32}),
        ),
        (
            "/v1/completions",
            json!({"model":"fixture","prompt":"西片的男朋友是谁？","max_tokens":32}),
        ),
        (
            "/v1beta/models/fixture:generateContent",
            json!({"contents":[{"role":"user","parts":[{"text":"西片的男朋友是谁？"}]}],"generationConfig":{"maxOutputTokens":32}}),
        ),
        (
            "/v2/chat",
            json!({"model":"fixture","messages":[{"role":"user","content":"西片的男朋友是谁？"}],"max_tokens":32}),
        ),
        (
            "/model/fixture/converse",
            json!({"messages":[{"role":"user","content":[{"text":"西片的男朋友是谁？"}]}],"inferenceConfig":{"maxTokens":32}}),
        ),
    ]
}

fn config(root: &str) -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"pool","adapter":"openai_compatible","base_url":root,
        "credentials":[{"id":"account","api_key":"fixture-key","discovery":{
            "source_url":root,"api_base":format!("{root}/v1"),"protocol":"chat_completions",
            "models":["fixture"],"verified_models":["fixture"],"checked_at":"2026-10-09T00:00:00Z",
            "binding":binding(root,"fixture-key"),"protocols":[
                {"protocol":"chat_completions","api_base":format!("{root}/v1"),"verified_models":["fixture"]},
                {"protocol":"messages","api_base":format!("{root}/v1"),"verified_models":["fixture"]}
            ]}}]}],"model_routes":[]})).unwrap()
}

#[tokio::test]
async fn override_is_closed_without_development_build_gate() {
    if cfg!(all(feature = "dev-protocol-override", debug_assertions)) {
        return;
    }
    let fixture = ConsoleStateFixture::new(config("https://fixture.test"), true);
    let response = build_router(fixture.state.clone())
        .oneshot(
            Request::post("/v1/chat/completions")
                .header("authorization", "Bearer public-api-contract-secret")
                .header("content-type", "application/json")
                .header("x-gateway-debug-upstream-protocol", "anthropic_messages")
                .body(Body::from(inputs()[0].1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        parse_json(response).await["error"]["code"],
        "debug_protocol_override_disabled"
    );
}

#[cfg(all(feature = "dev-protocol-override", debug_assertions))]
#[tokio::test]
async fn seven_ingress_formats_reach_both_targets_with_output_limits() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut upstream = Router::new();
    for path in ["/v1/chat/completions", "/v1/messages"] {
        let captured = captured.clone();
        upstream = upstream.route(path, post(move |headers: axum::http::HeaderMap, Json(body): Json<Value>| {
            let captured = captured.clone();
            async move {
                assert!(!headers.contains_key("x-gateway-debug-upstream-protocol"));
                let stream = body["stream"] == true;
                captured.lock().unwrap().push((path, body));
                if path.ends_with("/messages") && stream {
                    let events = [
                        ("message_start", json!({"type":"message_start","message":{"id":"fixture-msg","type":"message","role":"assistant","model":"fixture","content":[],"usage":{"input_tokens":1,"output_tokens":0}}})),
                        ("content_block_start", json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}})),
                        ("content_block_delta", json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"OK"}})),
                        ("content_block_stop", json!({"type":"content_block_stop","index":0})),
                        ("message_delta", json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}})),
                        ("message_stop", json!({"type":"message_stop"})),
                    ].into_iter().map(|(event, data)| format!("event: {event}\ndata: {data}\n\n")).collect::<String>();
                    return ([("content-type", "text/event-stream")], events).into_response();
                }
                Json(if path.ends_with("/messages") {
                    json!({"id":"fixture-msg","type":"message","role":"assistant","model":"fixture","content":[{"type":"text","text":"OK"}],"stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}})
                } else {
                    json!({"id":"fixture-chat","object":"chat.completion","model":"fixture","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}})
                }).into_response()
            }
        }));
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    let fixture = ConsoleStateFixture::new(config(&root), true);
    for target in ["openai_chat", "anthropic_messages"] {
        for (path, body) in inputs() {
            let response = build_router(fixture.state.clone())
                .oneshot(
                    Request::post(path)
                        .header("authorization", "Bearer public-api-contract-secret")
                        .header("content-type", "application/json")
                        .header("x-gateway-debug-upstream-protocol", target)
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let reply = parse_json(response).await;
            assert_eq!(status, StatusCode::OK, "{target} {path}: {reply}");
            assert!(reply.to_string().contains("OK"), "{target} {path}: {reply}");
            let requests = captured.lock().unwrap();
            let (actual, sent) = requests.last().unwrap();
            assert_eq!(
                *actual,
                if target == "openai_chat" {
                    "/v1/chat/completions"
                } else {
                    "/v1/messages"
                }
            );
            assert_eq!(sent["max_tokens"], 32, "{target} {path}: {sent}");
            assert!(sent["messages"].to_string().contains("西片的男朋友是谁？"));
            assert!(
                sent.get("inferenceConfig").is_none() && sent.get("generationConfig").is_none()
            );
        }
    }
    assert_eq!(captured.lock().unwrap().len(), 14);
    let response = build_router(fixture.state.clone())
        .oneshot(
            Request::post("/v1/chat/completions")
                .header("authorization", "Bearer public-api-contract-secret")
                .header("content-type", "application/json")
                .header("x-gateway-debug-upstream-protocol", "bedrock_converse")
                .body(Body::from(inputs()[0].1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        parse_json(response).await["error"]["code"],
        "debug_upstream_protocol_unavailable"
    );
    assert_eq!(
        captured.lock().unwrap().len(),
        14,
        "Must not silently fall back to Chat"
    );
    task.abort();
    let _ = task.await;
}
