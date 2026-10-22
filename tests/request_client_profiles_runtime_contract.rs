//! Actual buffered and SSE sends must share the configured wire identity (loopback only).
use axum::{
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use neuro_gateway::{
    protocol::{
        anthropic::normalize_messages, openai::normalize_chat_completions,
        responses::normalize_responses,
    },
    routing::candidate::ProviderAccountPayload,
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Captured = Arc<Mutex<Vec<(HeaderMap, Value)>>>;

async fn receive(
    State(captured): State<Captured>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    captured.lock().unwrap().push((headers, body.clone()));
    let model = body["model"].as_str().unwrap_or("fixture");
    if body["stream"] == true {
        let events = if body.get("input").is_some() {
            format!(
                "event: response.completed\ndata: {}\n\n",
                json!({"type":"response.completed","response":responses_reply(model)})
            )
        } else if body.get("max_tokens").is_some() {
            format!("event: message_start\ndata: {}\n\nevent: message_delta\ndata: {}\n\nevent: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n",
                json!({"type":"message_start","message":{"id":"msg","type":"message","role":"assistant","model":model,"content":[],"usage":{"input_tokens":1,"output_tokens":0}}}),
                json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}))
        } else {
            format!(
                "data: {}\n\ndata: [DONE]\n\n",
                json!({"id":"chat","object":"chat.completion.chunk","model":model,"choices":[{"index":0,"delta":{"content":"ok"},"finish_reason":"stop"}]})
            )
        };
        return ([("content-type", "text/event-stream")], events).into_response();
    }
    Json(if body.get("input").is_some() { responses_reply(model) } else {
        json!({"id":"chat","object":"chat.completion","model":model,"choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}})
    }).into_response()
}

fn responses_reply(model: &str) -> Value {
    json!({"id":"resp","object":"response","status":"completed","model":model,
        "output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"ok"}]}],
        "usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}})
}

#[tokio::test]
async fn request_client_profiles_reach_buffered_and_sse_transports() {
    let captured = Captured::default();
    let app = Router::new()
        .route("/v1/responses", post(receive))
        .route("/v1/chat/completions", post(receive))
        .route("/v1/messages", post(receive))
        .with_state(captured.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = UpstreamClient::new(5);
    for profile in ["auto", "codex_cli", "claude_cli", "workbuddy", "cpa"] {
        for model in ["gpt-fixture", "claude-sonnet-fixture", "gemini-fixture"] {
            for surface in ["responses", "chat", "messages"] {
                let adapter = if surface == "messages" {
                    "anthropic_compatible"
                } else {
                    "openai_compatible"
                };
                let payload: ProviderAccountPayload =
                    serde_json::from_value(json!({"adapter":adapter,
                    "base_url":base,"api_key":"selected-fixture","request_client_profile":profile}))
                    .unwrap();
                let req = match surface {
                    "responses" => normalize_responses(json!({"model":model,"input":"hello","instructions":"business"})),
                    "messages" => normalize_messages(json!({"model":model,"messages":[{"role":"user","content":"hello"}],"max_tokens":8})),
                    _ => normalize_chat_completions(json!({"model":model,"messages":[{"role":"user","content":"hello"}]})),
                }.unwrap();
                for stream in [false, true] {
                    if stream {
                        let result = tokio::time::timeout(
                            Duration::from_secs(10),
                            client.execute_stream(&payload, &req, model, None),
                        )
                        .await
                        .unwrap();
                        assert!(
                            result.is_ok(),
                            "{profile} {model} {surface} SSE send failed"
                        );
                        drop(result);
                    } else {
                        let result = tokio::time::timeout(
                            Duration::from_secs(10),
                            client.execute(&payload, &req, model, None),
                        )
                        .await
                        .unwrap();
                        assert!(
                            result.is_ok(),
                            "{profile} {model} {surface} buffered send: {:?}",
                            result.err()
                        );
                    }
                    let (headers, body) = captured.lock().unwrap().last().unwrap().clone();
                    let claude = profile == "claude_cli"
                        || (matches!(profile, "auto" | "cpa") && model.starts_with("claude-"));
                    let prefix = if profile == "workbuddy" {
                        "WorkBuddy/"
                    } else if claude {
                        "claude-cli/"
                    } else if profile == "cpa" {
                        "codex-tui/"
                    } else {
                        "codex_exec/"
                    };
                    assert!(headers["user-agent"].to_str().unwrap().starts_with(prefix));
                    if profile == "cpa" {
                        assert_eq!(
                            headers["accept"],
                            if stream {
                                "text/event-stream"
                            } else {
                                "application/json"
                            }
                        );
                        assert_eq!(
                            headers["accept-encoding"],
                            if claude {
                                if stream {
                                    "identity"
                                } else {
                                    "gzip, deflate, br, zstd"
                                }
                            } else {
                                "gzip"
                            }
                        );
                        assert!(!headers.contains_key("version"));
                        assert!(!headers.contains_key("sec-ch-ua"));
                        assert!(!headers.contains_key("sec-fetch-mode"));
                        if claude {
                            assert_eq!(headers["user-agent"], "claude-cli/2.1.280 (external, cli)");
                            assert_eq!(headers["x-stainless-os"], "MacOS");
                        }
                    }
                    if profile == "workbuddy" {
                        assert_eq!(headers["x-ide-type"], "WorkBuddy");
                        for name in [
                            "sec-ch-ua",
                            "sec-ch-ua-mobile",
                            "sec-ch-ua-platform",
                            "sec-fetch-dest",
                            "sec-fetch-user",
                        ] {
                            assert!(
                                !headers.contains_key(name),
                                "browser identity leaked: {name}"
                            );
                        }
                    }
                    assert_eq!(body["model"], model);
                    assert!(body.get("request_client_profile").is_none());
                    if surface == "responses" {
                        assert_eq!(body["instructions"], "business");
                        assert_eq!(body["stream"], stream);
                        if !claude && profile != "workbuddy" {
                            assert_eq!(body["store"], false);
                        }
                    }
                    assert_eq!(headers.contains_key("authorization"), surface != "messages");
                    assert_eq!(headers.contains_key("x-api-key"), surface == "messages");
                }
            }
        }
    }
    assert_eq!(captured.lock().unwrap().len(), 90);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
}
