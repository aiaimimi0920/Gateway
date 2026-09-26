use super::frames::{build_openai_sse_frames, parse_frame_text};
use super::*;
use crate::protocol::canonical::TokenUsage;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;
use rquest::Method;
use serde_json::json;
use std::collections::HashMap;

fn make_payload() -> ProviderAccountPayload {
    ProviderAccountPayload {
        adapter: "xfyun_websocket_compatible".to_string(),
        base_url: "wss://maas-api.cn-huabei-1.xf-yun.com".to_string(),
        api_key: "api-key-123".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some("xop35qwen2b".to_string()),
        headers: HashMap::new(),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: Some("api-secret-456".to_string()),
        responses_path: None,
        chat_completions_path: Some("/v1.1/chat".to_string()),
        completions_path: None,
        embeddings_path: None,
        audio_transcriptions_path: None,
        audio_speech_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        extra_body: Some(HashMap::from([
            ("appId".to_string(), json!("app-id-789")),
            ("uid".to_string(), json!("gateway-test")),
        ])),
        session_auth: None,
        keepalive: None,
    }
}

fn make_request() -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("qwen3.5-35b-a3b".to_string()),
        stream: true,
        messages: vec![
            CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text {
                    text: "You are terse.".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Reply with exactly: native websocket ok".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
        ],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "temperature": 0.1,
            "max_tokens": 256,
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    }
}

#[test]
fn packs_native_request_body() {
    let body = pack_request(&make_payload(), &make_request(), "xop35qwen2b").unwrap();
    assert_eq!(body["header"]["app_id"], "app-id-789");
    assert_eq!(body["parameter"]["chat"]["domain"], "xop35qwen2b");
    assert_eq!(body["parameter"]["chat"]["temperature"], json!(0.1));
    assert_eq!(body["parameter"]["chat"]["max_tokens"], json!(256));
    assert_eq!(body["payload"]["message"]["text"][0]["role"], "system");
    assert_eq!(body["payload"]["message"]["text"][1]["role"], "user");
}

#[test]
fn builds_signed_websocket_url() {
    let url = build_signed_websocket_url(&make_payload()).unwrap();
    assert!(url.starts_with("wss://maas-api.cn-huabei-1.xf-yun.com/v1.1/chat?"));
    assert!(url.contains("authorization="));
    assert!(url.contains("date="));
    assert!(url.contains("host="));
}

fn assert_request_plan_contract(plan: &RequestPlan) {
    assert_eq!(plan.method, Method::POST);
    assert!(plan
        .url
        .starts_with("wss://maas-api.cn-huabei-1.xf-yun.com/v1.1/chat?"));
    assert!(plan.url.contains("authorization="));
    assert_eq!(plan.response_kind, EndpointKind::ChatCompletions);
    assert_eq!(
        plan.body.as_ref().unwrap()["header"]["app_id"],
        "app-id-789"
    );
    assert_eq!(
        plan.body.as_ref().unwrap()["parameter"]["chat"]["domain"],
        "xop35qwen2b"
    );
}

#[test]
fn xfyun_websocket_request_plan_builds_signed_url_and_native_body() {
    let plan = build_request_plan(&make_payload(), &make_request(), "xop35qwen2b").unwrap();
    assert_request_plan_contract(&plan);
}

#[test]
fn xfyun_websocket_request_plan_rejects_unsupported_endpoint() {
    let mut req = make_request();
    req.endpoint_kind = EndpointKind::Embeddings;
    let err = build_request_plan(&make_payload(), &req, "xop35qwen2b")
        .expect_err("xfyun websocket should reject embeddings");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(
        err.code.as_deref(),
        Some("unsupported_xfyun_websocket_endpoint")
    );
}

#[test]
fn parses_success_frame() {
    let parsed = parse_frame_text(
        r#"{
            "header": {"code": 0},
            "payload": {
                "choices": {
                    "status": 2,
                    "text": [{"content": "hello websocket"}]
                },
                "usage": {
                    "text": {
                        "prompt_tokens": 8,
                        "completion_tokens": 4,
                        "total_tokens": 12
                    }
                }
            }
        }"#,
        "xop35qwen2b",
    )
    .unwrap();
    assert_eq!(
        parsed.content_fragments,
        vec!["hello websocket".to_string()]
    );
    assert!(parsed.done);
    assert_eq!(
        parsed.usage.as_ref().map(|usage| usage.total_tokens),
        Some(12)
    );
}

#[test]
fn ignores_empty_intermediate_frame() {
    let parsed = parse_frame_text(
        r#"{
            "header": {"code": 0},
            "payload": {
                "choices": {
                    "status": 0,
                    "text": []
                }
            }
        }"#,
        "xop35qwen2b",
    )
    .unwrap();
    assert!(parsed.content_fragments.is_empty());
    assert!(parsed.usage.is_none());
    assert!(!parsed.done);
}

#[test]
fn converts_frame_to_openai_sse() {
    let parsed = ParsedXfyunFrame {
        content_fragments: vec!["delta".to_string()],
        usage: Some(TokenUsage {
            prompt_tokens: 3,
            completion_tokens: 2,
            total_tokens: 5,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        }),
        done: true,
    };
    let frames = build_openai_sse_frames(&parsed, "xop35qwen2b", "chatcmpl-test", 123);
    let joined = frames
        .into_iter()
        .map(|bytes| String::from_utf8(bytes.to_vec()).unwrap())
        .collect::<Vec<_>>()
        .join("");
    assert!(joined.contains("\"chatcmpl-test\""));
    assert!(joined.contains("\"delta\""));
    assert!(joined.contains("\"finish_reason\":\"stop\""));
    assert!(joined.contains("data: [DONE]"));
}
