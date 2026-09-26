use super::*;
use crate::pipeline::PipelineContext;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use serde_json::json;
use std::collections::HashMap;

fn make_ctx() -> PipelineContext {
    PipelineContext::new(
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("gpt-4o".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hi".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        },
        None,
    )
}

#[test]
fn extract_usage_openai_shape() {
    let body = json!({
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 20,
            "total_tokens": 30
        }
    });
    let usage = extract_usage(&body, &Some("openai_compatible".to_string())).unwrap();
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 20);
    assert_eq!(usage.total_tokens, 30);
    assert_eq!(usage.cache_creation_input_tokens, None);
    assert_eq!(usage.cache_read_input_tokens, None);
}

#[test]
fn extract_usage_anthropic_shape() {
    let body = json!({
        "usage": {
            "input_tokens": 15,
            "output_tokens": 25,
            "cache_creation_input_tokens": 200,
            "cache_read_input_tokens": 120
        }
    });
    let usage = extract_usage(&body, &Some("anthropic_compatible".to_string())).unwrap();
    assert_eq!(usage.prompt_tokens, 15);
    assert_eq!(usage.completion_tokens, 25);
    assert_eq!(usage.total_tokens, 40);
    assert_eq!(usage.cache_creation_input_tokens, Some(200));
    assert_eq!(usage.cache_read_input_tokens, Some(120));
}

#[test]
fn extract_usage_missing_returns_zeros() {
    let body = json!({ "choices": [] });
    assert!(extract_usage(&body, &Some("openai_compatible".to_string())).is_none());
}

#[test]
fn extract_usage_openai_infers_total() {
    let body = json!({
        "usage": {
            "prompt_tokens": 5,
            "completion_tokens": 7
        }
    });
    let usage = extract_usage(&body, &Some("openai_compatible".to_string())).unwrap();
    assert_eq!(usage.total_tokens, 12);
}

#[test]
fn format_unix_secs_as_iso_format() {
    // 2026-04-07T00:00:00Z  →  unix = 1744156800 (approximately)
    // We just check the format is correct length and ends with Z.
    let s = format_unix_secs_as_iso(1_744_156_800);
    assert!(s.ends_with('Z'), "expected Z suffix, got: {s}");
    assert_eq!(s.len(), 20, "unexpected format: {s}");
}

#[test]
fn extract_response_id_reads_top_level_id() {
    let body = json!({
        "id": "resp_123",
        "object": "response"
    });
    assert_eq!(extract_response_id(&body).as_deref(), Some("resp_123"));
}

#[test]
fn route_trace_includes_first_class_protocol_semantics() {
    let mut ctx = make_ctx();
    ctx.selected_provider_id = Some("prov-1".to_string());
    ctx.selected_provider_label = Some("Provider".to_string());
    ctx.selected_adapter = Some("openai_compatible".to_string());
    ctx.selected_execution_mode = Some("direct_http".to_string());
    ctx.selected_upstream_target_protocol_family = Some("openai_responses".to_string());
    ctx.selected_upstream_target_conversation_family = Some("openai_responses".to_string());
    ctx.canonical_conversation_semantics = Some("messages_turns".to_string());
    ctx.selected_upstream_target_tool_family = Some("xml_fallback".to_string());
    ctx.tool_strategy = Some("xml_fallback".to_string());
    ctx.canonical_tool_choice_semantics = Some("prompt_only".to_string());
    ctx.canonical_completion_semantics = Some("tool_calls".to_string());

    let trace = build_route_trace_from_ctx(&ctx, None);

    assert_eq!(
        trace["selectedUpstreamTargetProtocolFamily"],
        "openai_responses"
    );
    assert_eq!(
        trace["selectedUpstreamTargetConversationFamily"],
        "openai_responses"
    );
    assert_eq!(trace["canonicalConversationSemantics"], "messages_turns");
    assert_eq!(trace["selectedUpstreamTargetToolFamily"], "xml_fallback");
    assert_eq!(trace["toolStrategy"], "xml_fallback");
    assert_eq!(trace["canonicalToolChoiceSemantics"], "prompt_only");
    assert_eq!(trace["canonicalCompletionSemantics"], "tool_calls");
    assert_eq!(
        trace["selectedCandidate"]["canonicalToolChoiceSemantics"],
        "prompt_only"
    );
}

#[test]
fn route_trace_separates_retry_count_from_actual_provider_ids() {
    let ctx = make_ctx();
    ctx.note_provider_attempt("provider-b");
    ctx.note_provider_attempt("provider-b");

    let trace = build_route_trace_from_ctx(&ctx, None);

    assert_eq!(trace["routeAttemptCount"], 2);
    assert_eq!(trace["attemptedProviderIds"], json!(["provider-b"]));
}

#[test]
fn protocol_family_name_maps_openai_embeddings_to_explicit_family() {
    let mut ctx = make_ctx();
    ctx.canonical_req.endpoint_kind = EndpointKind::Embeddings;
    assert_eq!(protocol_family_name(&ctx), OPENAI_EMBEDDINGS_FAMILY);
}

#[test]
fn apply_stream_completion_semantics_updates_snapshot_route_trace() {
    let ctx = make_ctx();
    let mut snapshot = snapshot_request_audit(&ctx);
    assert_eq!(snapshot.canonical_completion_semantics, None);

    apply_stream_completion_semantics(&mut snapshot, Some("length".to_string()));
    let trace = build_route_trace_from_snapshot(&snapshot, None);

    assert_eq!(trace["canonicalCompletionSemantics"], "length");
}
