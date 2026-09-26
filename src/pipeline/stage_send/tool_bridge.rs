//! Candidate tool bridge ownership.
use super::*;

pub(super) fn capture_prompt_cache_telemetry(
    ctx: &mut PipelineContext,
    adapter: &str,
    model: &str,
) {
    let telemetry = match adapter {
        "anthropic_compatible" => {
            anthropic::inspect_prompt_cache_telemetry(&ctx.canonical_req, model, ctx.stream)
        }
        "accio_compatible" => accio::inspect_prompt_cache_telemetry(&ctx.canonical_req, model),
        _ => {
            ctx.client_has_cache_control = false;
            ctx.auto_cache_applied = false;
            return;
        }
    };

    ctx.client_has_cache_control = telemetry.client_has_cache_control;
    ctx.auto_cache_applied = telemetry.auto_cache_applied;
}

fn has_strict_tool_choice(req: &crate::protocol::canonical::CanonicalRelayRequest) -> bool {
    matches!(
        tool_choice::parse_tool_choice(req.tool_choice.as_ref()),
        Some(CanonicalToolChoice::Required | CanonicalToolChoice::Specific(_))
    )
}

pub(super) fn maybe_apply_xml_tool_response_bridge(
    req_id: &uuid::Uuid,
    canonical_resp: &mut crate::protocol::canonical::CanonicalRelayResponse,
    original_tools: &[crate::protocol::canonical::CanonicalTool],
    original_tool_choice: Option<&serde_json::Value>,
    original_messages_text: Option<&str>,
    tools_were_injected: bool,
) -> bool {
    if original_tools.is_empty() || !canonical_resp.tool_calls.is_empty() {
        return false;
    }

    let parse_result = tool_inject::parse_tool_calls_from_text_with_context(
        &canonical_resp.text,
        original_tools,
        original_tool_choice,
        original_messages_text,
    );
    if !parse_result.had_tool_calls {
        return false;
    }

    debug!(
        req_id = %req_id,
        count = parse_result.tool_calls.len(),
        tools_were_injected,
        "promoted XML tool calls from upstream text response"
    );
    canonical_resp.text = parse_result.clean_text;
    canonical_resp.tool_calls = parse_result.tool_calls;
    canonical_resp.finish_reason = Some("tool_calls".to_string());
    true
}

pub(super) fn should_force_bridge_tool_injection(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    adapter: &str,
) -> bool {
    if req.tools.is_empty() || adapter != "anthropic_compatible" || !has_strict_tool_choice(req) {
        return false;
    }

    matches!(
        (req.protocol_family, req.endpoint_kind),
        (
            ProtocolFamily::OpenAiRealtime,
            EndpointKind::ChatCompletions
        ) | (ProtocolFamily::OpenAi, EndpointKind::Responses)
    )
}
