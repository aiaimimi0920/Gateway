use super::*;
use crate::protocol::openai;

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(
        json!({"adapter":antigravity::ADAPTER,"base_url":antigravity::BASE_URL,
        "api_key":"synthetic-token","extra_body":{"antigravity_project_id":"actual-project"}}),
    )
    .unwrap()
}
#[test]
fn antigravity_cloudcode_envelope_is_owned_and_tool_policy_is_preserved() {
    let mut req = openai::normalize_chat_completions(json!({"model":"catalog-model","messages":[{"role":"user","content":"hello"}],
        "tools":[{"type":"function","function":{"name":"lookup","parameters":{"type":"object"}}}],"tool_choice":"none"})).unwrap();
    req.extra
        .insert("project".into(), json!("attacker-project"));
    req.extra
        .insert("sessionId".into(), json!("attacker-session"));
    let value = wire::envelope(&payload(), &req, "catalog-model").unwrap();
    assert_eq!(value["project"], "actual-project");
    assert_eq!(
        value["request"]["toolConfig"]["functionCallingConfig"]["mode"],
        "NONE"
    );
    assert_ne!(value["request"]["sessionId"], "attacker-session");
    assert!(value["request"].get("project").is_none());
    assert!(value["request"].get("model").is_none());
    let second = wire::envelope(&payload(), &req, "catalog-model").unwrap();
    assert_ne!(value["requestId"], second["requestId"]);
}
#[test]
fn antigravity_response_unwrap_and_sse_preserve_text_tools_and_usage() {
    let bytes = concat!(
        "data: {\"response\":{\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hello\"}]}}]}}\r\n\r\n",
        "data: {\"response\":{\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"lookup\",\"args\":{\"x\":1}}}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":3,\"candidatesTokenCount\":2,\"totalTokenCount\":5}}}\r\n\r\n"
    );
    let value = response::decode(bytes.as_bytes(), true).unwrap();
    let response =
        crate::protocol::gemini_api::parse_generate_content_response(&value, "catalog-model")
            .unwrap();
    assert_eq!(response.text, "hello");
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.usage.unwrap().total_tokens, 5);
    assert!(response::decode(b"data: {\"response\":{\"candidates\":[]}}\n\n", true).is_err());
    assert!(response::decode(b"{\"candidates\":[]}", false).is_err());
    assert!(response::decode(
        b"{\"response\":{\"error\":{\"message\":\"secret\"}}}",
        false
    )
    .is_err());
}
#[tokio::test]
#[cfg(feature = "cpa-rust-transport")]
async fn antigravity_final_send_uses_dedicated_headers_not_chrome_defaults() {
    use axum::{routing::post, Router};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = std::sync::Arc::new(parking_lot::Mutex::new(Some(tx)));
    let app = Router::new().route(
        "/",
        post(move |headers: axum::http::HeaderMap| {
            let tx = tx.clone();
            async move {
                if let Some(tx) = tx.lock().take() {
                    let _ = tx.send(headers);
                }
                "{}"
            }
        }),
    );
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut defaults = rquest::header::HeaderMap::new();
    defaults.insert("sec-ch-ua", "Chrome".parse().unwrap());
    defaults.insert("x-goog-user-project", "must-not-leak".parse().unwrap());
    defaults.insert("cookie", "must-not-leak".parse().unwrap());
    let client = rquest::Client::builder()
        .no_proxy()
        .default_headers(defaults)
        .build()
        .unwrap();
    let request = fingerprint::apply(client.post(format!("http://{address}/")), "synthetic")
        .json(&json!({}))
        .build()
        .unwrap();
    let transport = gateway_cpa_http_transport::Client::with_profile(
        gateway_cpa_http_transport::Profile::AntigravityHub215,
        None,
        true,
    )
    .unwrap();
    let response = transport
        .send(request, Duration::from_secs(3))
        .await
        .unwrap();
    assert!(response.status().is_success());
    let headers = rx.await.unwrap();
    task.abort();
    assert_eq!(headers["user-agent"], fingerprint::USER_AGENT);
    assert_eq!(headers["authorization"], "Bearer synthetic");
    assert_eq!(headers["accept-encoding"], "gzip");
    for name in [
        "accept",
        "sec-ch-ua",
        "cookie",
        "x-goog-user-project",
        "origin",
    ] {
        assert!(!headers.contains_key(name));
    }
}
#[test]
fn antigravity_does_not_invent_projects_catalogs_or_accept_custom_origins() {
    assert!(catalog_models(&json!({"models":{}})).is_err());
    assert_eq!(
        catalog_models(&json!({"models":{"b":{},"a":{}}})).unwrap(),
        vec!["a", "b"]
    );
    for base in [
        "http://cloudcode-pa.googleapis.com/v1internal",
        "https://evil.invalid/v1internal",
        "https://cloudcode-pa.googleapis.com/v1internal?x=1",
    ] {
        assert!(transport::admit_base(base).is_err());
    }
    let mut payload = payload();
    payload.extra_body = None;
    let req =
        openai::normalize_chat_completions(json!({"messages":[{"role":"user","content":"test"}]}))
            .unwrap();
    assert_eq!(
        wire::envelope(&payload, &req, "model")
            .unwrap_err()
            .code
            .as_deref(),
        Some("antigravity_project_missing")
    );
}

#[test]
fn antigravity_rejects_partial_empty_blocked_and_unterminated_results() {
    for reason in [
        Value::Null,
        json!(""),
        json!(12),
        json!({}),
        json!("UNKNOWN"),
    ] {
        let body = json!({"response":{"candidates":[{"content":{"parts":[{"text":"partial"}]},"finishReason":reason}]}});
        let bytes = serde_json::to_vec(&body).unwrap();
        assert!(response::decode(&bytes, false).is_err());
        let event = format!("data: {body}\n\n");
        assert!(response::decode(event.as_bytes(), true).is_err());
    }
    for body in [
        json!({"response":{"candidates":[]}}),
        json!({"response":{"candidates":[{"content":{"parts":[{"text":"partial"}]}}]}}),
        json!({"response":{"candidates":[{"content":{"parts":[]},"finishReason":"STOP"}]}}),
        json!({"response":{"promptFeedback":{"blockReason":"SAFETY"},"candidates":[]}}),
    ] {
        assert!(response::decode(&serde_json::to_vec(&body).unwrap(), false).is_err());
    }
    let full=json!({"response":{"candidates":[{"content":{"parts":[{"text":"complete"}]},"finishReason":"STOP"}]}}).to_string();
    assert!(response::decode(full.as_bytes(), false).is_ok());
    assert!(response::decode(format!("data: {full}\n").as_bytes(), true).is_err());
    assert!(response::decode(format!("data: {full}\n\n").as_bytes(), true).is_ok());
}

#[test]
fn antigravity_stream_cannot_replace_a_rejected_or_prior_terminal() {
    let content = json!({"response":{"candidates":[{"content":{"parts":[{"text":"partial"}]}}]}});
    let stop = json!({"response":{"candidates":[{"finishReason":"STOP"}]}});
    for reason in [
        Value::Null,
        json!(12),
        json!("UNKNOWN"),
        json!("SAFETY"),
        json!("STOP"),
    ] {
        let terminal = json!({"response":{"candidates":[{"finishReason":reason}]}});
        let events = format!("data: {content}\n\ndata: {terminal}\n\ndata: {stop}\n\n");
        assert!(
            response::decode(events.as_bytes(), true).is_err(),
            "{reason}"
        );
    }
}
