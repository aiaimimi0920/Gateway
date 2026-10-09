//! Explicit opt-in real-provider smoke; secrets stay in environment and memory.
#![cfg(all(feature = "dev-protocol-override", debug_assertions))]
use axum::{body::Body, http::Request};
use futures::{stream, StreamExt};
use neuro_gateway::{
    http::router::build_router,
    provider_discovery::binding,
    routing::config::{RouteConfigStore, RouteConfigYaml},
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Instant};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;

fn requests() -> Vec<(&'static str, &'static str, Value)> {
    let prompt = "西片的男朋友是谁？";
    vec![
        (
            "chat",
            "/v1/chat/completions",
            json!({"model":"grok-4.7","messages":[{"role":"user","content":prompt}],"max_tokens":512}),
        ),
        (
            "responses",
            "/v1/responses",
            json!({"model":"grok-4.7","input":prompt,"max_output_tokens":512}),
        ),
        (
            "messages",
            "/v1/messages",
            json!({"model":"grok-4.7","messages":[{"role":"user","content":prompt}],"max_tokens":512}),
        ),
        (
            "completions",
            "/v1/completions",
            json!({"model":"grok-4.7","prompt":prompt,"max_tokens":512}),
        ),
        (
            "gemini",
            "/v1beta/models/grok-4.7:generateContent",
            json!({"contents":[{"role":"user","parts":[{"text":prompt}]}],"generationConfig":{"maxOutputTokens":512}}),
        ),
        (
            "cohere",
            "/v2/chat",
            json!({"model":"grok-4.7","messages":[{"role":"user","content":prompt}],"max_tokens":512}),
        ),
        (
            "bedrock",
            "/model/grok-4.7/converse",
            json!({"messages":[{"role":"user","content":[{"text":prompt}]}],"inferenceConfig":{"maxTokens":512}}),
        ),
    ]
}

fn text(kind: &str, body: &Value) -> String {
    match kind {
        "chat" => body
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        "completions" => body
            .pointer("/choices/0/text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        "responses" => body["output"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|o| o["content"].as_array().into_iter().flatten())
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join(""),
        _ => {
            let path = match kind {
                "messages" => "/content",
                "gemini" => "/candidates/0/content/parts",
                "cohere" => "/message/content",
                _ => "/output/message/content",
            };
            body.pointer(path)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join("")
        }
    }
}

#[tokio::test]
#[ignore = "Real upstream generation requires explicit user approval and GATEWAY_HY_PROBE_KEY"]
async fn live_hy_seven_inputs_to_both_targets() {
    let key = std::env::var("GATEWAY_HY_PROBE_KEY").expect("upstream key environment required");
    let public_key =
        std::env::var("GATEWAY_TEST_KEY").expect("local client key environment required");
    let output = std::env::var("GATEWAY_MATRIX_OUTPUT").expect("evidence path required");
    let root = "https://ai.hybgzs.com";
    let config: RouteConfigYaml = serde_json::from_value(json!({"providers":[{"id":"hy-live-test","adapter":"openai_compatible","base_url":root,
        "credentials":[{"id":"hy-live-test-account","api_key":key,"discovery":{
            "source_url":root,"api_base":format!("{root}/v1"),"protocol":"chat_completions",
            "models":["grok-4.7"],"verified_models":["grok-4.7"],"checked_at":"2026-10-09T00:00:00Z",
            "binding":binding(root,&key),"protocols":[
                {"protocol":"chat_completions","api_base":format!("{root}/v1"),"verified_models":["grok-4.7"]},
                {"protocol":"messages","api_base":format!("{root}/v1"),"verified_models":["grok-4.7"]}
            ]}}]}],"model_routes":[]})).unwrap();
    let mut state = support::build_state(
        Arc::new(RouteConfigStore::from_document(config).unwrap()),
        None,
    );
    let mutable = Arc::get_mut(&mut state).unwrap();
    mutable.config.gateway_api_key = Some(public_key.clone());
    mutable.config.upstream_timeout_secs = 160;
    mutable.upstream_client = UpstreamClient::new(160);
    let cases = ["openai_chat", "anthropic_messages"]
        .into_iter()
        .flat_map(|target| {
            requests()
                .into_iter()
                .map(move |(kind, path, body)| (target, kind, path, body))
        })
        .collect::<Vec<_>>();
    let results = stream::iter(cases).map(|(target, kind, path, body)| {
        let state = state.clone(); let public_key = public_key.clone();
        async move {
            let started = Instant::now();
            let request = Request::post(path).header("authorization", format!("Bearer {public_key}"))
                .header("content-type", "application/json").header("anthropic-version", "2023-06-01")
                .header("x-gateway-debug-upstream-protocol", target)
                .body(Body::from(body.to_string())).unwrap();
            let result = tokio::time::timeout(std::time::Duration::from_secs(330), async {
                let response = build_router(state).oneshot(request).await.unwrap();
                let status = response.status().as_u16();
                let body = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
                (status, serde_json::from_slice::<Value>(&body).unwrap_or_else(|_| json!({"error":"invalid JSON"})))
            }).await;
            let (status, body) = result.unwrap_or((0, json!({"error":"330 second test deadline exceeded"})));
            let answer = text(kind, &body);
            let record = json!({"input":kind,"target":target,"endpoint":path,"status":status,
                "seconds":started.elapsed().as_secs_f64(),"valid":status==200 && !answer.trim().is_empty(),"answer":answer,"response":body});
            println!("{}", json!({"input":kind,"target":target,"status":status,"seconds":record["seconds"],"valid":record["valid"]}));
            record
        }
    }).buffer_unordered(2).collect::<Vec<_>>().await;
    let sanitized = serde_json::to_string_pretty(&results)
        .unwrap()
        .replace(&key, "[REDACTED]")
        .replace(&public_key, "[REDACTED]");
    std::fs::write(output, sanitized).unwrap();
    assert_eq!(
        results.iter().filter(|r| r["valid"] == true).count(),
        14,
        "Inspect redacted matrix evidence"
    );
}
