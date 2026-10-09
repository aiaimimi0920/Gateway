//! Production HTTP ingress and dispatch against an isolated, credential-free upstream.
use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
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
const TEXT: &str = "/api/v1/services/aigc/text-generation/generation";
const MULTI: &str = "/api/v1/services/aigc/multimodal-generation/generation";

fn config(root: &str, protocols: &[&str]) -> RouteConfigYaml {
    let entries = protocols.iter().map(|p| json!({"protocol":p,"api_base":format!("{root}{}", if p.starts_with("dashscope") {"/api/v1"} else {"/v1"}),"verified_models":["fixture"]})).collect::<Vec<_>>();
    serde_json::from_value(json!({"providers":[{"id":"pool","adapter":"openai_compatible","base_url":root,
        "credentials":[{"id":"account","api_key":"fixture-key","discovery":{
        "source_url":root,"api_base":entries[0]["api_base"],"protocol":protocols[0],"models":["fixture"],"verified_models":["fixture"],"checked_at":"2026-10-09T00:00:00Z","binding":binding(root,"fixture-key"),"protocols":entries}}]}],"model_routes":[]})).unwrap()
}
fn native_body(multimodal: bool) -> Value {
    json!({"model":"fixture","input":{"messages":[{"role":"user","content":if multimodal {json!([{"text":"hello"}])} else {json!("hello")} }]},"parameters":{"max_tokens":32,"result_format":"message","incremental_output":true}})
}
fn request(path: &str, body: Value, stream: bool) -> Request<Body> {
    let mut req = Request::post(path)
        .header("authorization", "Bearer public-api-contract-secret")
        .header("content-type", "application/json");
    if stream {
        req = req.header("x-dashscope-sse", "enable");
    }
    req.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn dashscope_bidirectional_and_native_http() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut upstream = Router::new();
    for path in [TEXT, MULTI, "/v1/chat/completions", "/v1/messages"] {
        let captured = captured.clone();
        upstream = upstream.route(path,post(move |headers: HeaderMap, Json(body): Json<Value>| {
            let captured = captured.clone();
            async move {
                if path == "/v1/messages" { assert_eq!(headers["x-api-key"],"fixture-key"); }
                else { assert_eq!(headers["authorization"],"Bearer fixture-key"); }
                let streaming = headers.get("x-dashscope-sse").is_some() || body["stream"] == true;
                captured.lock().unwrap().push((path,body.clone()));
                if path == "/v1/messages" {
                    let events = [
                        ("message_start",json!({"type":"message_start","message":{"id":"fixture","type":"message","role":"assistant","model":"fixture","content":[],"usage":{"input_tokens":2,"output_tokens":0}}})),
                        ("content_block_start",json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}})),
                        ("content_block_delta",json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"OK"}})),
                        ("content_block_stop",json!({"type":"content_block_stop","index":0})),
                        ("message_delta",json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}})),
                        ("message_stop",json!({"type":"message_stop"})),
                    ].into_iter().map(|(event,data)|format!("event: {event}\ndata: {data}\n\n")).collect::<String>();
                    return ([("content-type","text/event-stream")],events).into_response();
                }
                let chat = json!({"id":"fixture","model":"fixture","object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}});
                let native = if body.pointer("/parameters/ocr_options").is_some() {
                    json!({"request_id":"native","output":{"choices":[{"message":{"content":[{"ocr_result":{"boxes":[1,2,3]}}]},"finish_reason":"stop"}]},"usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3}})
                } else {
                    neuro_gateway::protocol::dashscope::response::from_openai(&chat,path == MULTI,true).unwrap()
                };
                if streaming {
                    let event = if path == "/v1/chat/completions" {
                        format!("data: {}\n\ndata: [DONE]\n\n",json!({"id":"fixture","model":"fixture","choices":[{"index":0,"delta":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}}))
                    } else { format!("event: result\ndata: {native}\n\n") };
                    return ([("content-type","text/event-stream")],event).into_response();
                }
                Json(if path == "/v1/chat/completions" {chat} else {native}).into_response()
            }
        }));
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    for (protocol, path) in [("dashscope_text", TEXT), ("dashscope_multimodal", MULTI)] {
        let fixture = ConsoleStateFixture::new(config(&root, &[protocol]), true);
        for (ingress, body) in [
            (
                "/v1/messages",
                json!({"model":"fixture","messages":[{"role":"user","content":"hello"}],"max_tokens":32}),
            ),
            (
                "/v1/responses",
                json!({"model":"fixture","input":"hello","max_output_tokens":32}),
            ),
            (
                "/v1/completions",
                json!({"model":"fixture","prompt":"hello","max_tokens":32}),
            ),
            (
                "/v1beta/models/fixture:generateContent",
                json!({"contents":[{"role":"user","parts":[{"text":"hello"}]}],"generationConfig":{"maxOutputTokens":32}}),
            ),
            (
                "/v2/chat",
                json!({"model":"fixture","messages":[{"role":"user","content":"hello"}],"max_tokens":32}),
            ),
            (
                "/model/fixture/converse",
                json!({"messages":[{"role":"user","content":[{"text":"hello"}]}],"inferenceConfig":{"maxTokens":32}}),
            ),
        ] {
            let response = build_router(fixture.state.clone())
                .oneshot(request(ingress, body, false))
                .await
                .unwrap();
            let status = response.status();
            let reply = parse_json(response).await;
            assert_eq!(status, StatusCode::OK, "{protocol} {ingress}: {reply}");
            assert!(reply.to_string().contains("OK"));
            assert_eq!(
                captured.lock().unwrap().last().unwrap().1["parameters"]["max_tokens"],
                32
            );
        }
        for stream in [false, true] {
            let chat = json!({"model":"fixture","messages":[{"role":"user","content":"hello"}],"max_tokens":32,"stream":stream});
            let response = build_router(fixture.state.clone())
                .oneshot(request("/v1/chat/completions", chat, false))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
                .await
                .unwrap();
            assert!(String::from_utf8_lossy(&bytes).contains("OK"));
            let sent = captured.lock().unwrap().last().cloned().unwrap();
            assert_eq!(sent.0, path);
            assert_eq!(sent.1["parameters"]["max_tokens"], 32);
            assert!(sent.1.get("stream").is_none());
            let response = build_router(fixture.state.clone())
                .oneshot(request(path, native_body(path == MULTI), stream))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
                .await
                .unwrap();
            assert!(String::from_utf8_lossy(&bytes).contains("output"));
        }
    }
    for target in ["chat_completions", "messages"] {
        let fixture = ConsoleStateFixture::new(config(&root, &[target]), true);
        for path in [TEXT, MULTI] {
            for stream in [false, true] {
                let response = build_router(fixture.state.clone())
                    .oneshot(request(path, native_body(path == MULTI), stream))
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
                    .await
                    .unwrap();
                let text = String::from_utf8_lossy(&bytes);
                assert!(text.contains("output") && text.contains("OK"), "{text}");
            }
        }
    }
    let fixture = ConsoleStateFixture::new(config(&root, &["chat_completions"]), true);
    let mut ocr = native_body(true);
    ocr["parameters"]["ocr_options"] = json!({"task":"advanced_recognition"});
    let before = captured.lock().unwrap().len();
    let response = build_router(fixture.state.clone())
        .oneshot(request(MULTI, ocr.clone(), false))
        .await
        .unwrap();
    assert!(!response.status().is_success());
    assert_eq!(
        captured.lock().unwrap().len(),
        before,
        "Lossy bridge must not call upstream"
    );
    let native = ConsoleStateFixture::new(
        config(&root, &["chat_completions", "dashscope_multimodal"]),
        true,
    );
    let response = build_router(native.state.clone())
        .oneshot(request(MULTI, ocr.clone(), false))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = parse_json(response).await;
    assert_eq!(
        result["output"]["choices"][0]["message"]["content"][0]["ocr_result"]["boxes"],
        json!([1, 2, 3])
    );
    assert_eq!(captured.lock().unwrap().last().unwrap().1, ocr);
    task.abort();
    let _ = task.await;
}
