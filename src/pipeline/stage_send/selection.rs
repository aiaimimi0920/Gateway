//! Candidate selection ownership.
use super::*;

fn selected_openai_target_family(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    candidate: &crate::routing::candidate::RouteCandidate,
) -> &'static str {
    if candidate
        .payload
        .bridges_openai_text_endpoint_to_responses(req.endpoint_kind)
    {
        "openai_responses"
    } else if candidate
        .payload
        .bridges_openai_responses_to_chat_completions(req.endpoint_kind)
    {
        "openai_chat"
    } else if req.endpoint_kind == EndpointKind::Embeddings {
        OPENAI_EMBEDDINGS_FAMILY
    } else if req.endpoint_kind == EndpointKind::AudioTranscriptions {
        OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY
    } else if req.endpoint_kind == EndpointKind::AudioSpeech {
        OPENAI_AUDIO_SPEECH_FAMILY
    } else if req.endpoint_kind == EndpointKind::ImagesGenerations {
        OPENAI_IMAGES_GENERATIONS_FAMILY
    } else if req.endpoint_kind == EndpointKind::ImagesEdits {
        OPENAI_IMAGES_EDITS_FAMILY
    } else if req.endpoint_kind == EndpointKind::MusicGenerations {
        OPENAI_MUSIC_GENERATIONS_FAMILY
    } else if req.endpoint_kind == EndpointKind::VideosGenerations {
        OPENAI_VIDEOS_GENERATIONS_FAMILY
    } else if req.endpoint_kind == EndpointKind::Responses {
        "openai_responses"
    } else {
        "openai_chat"
    }
}

fn infer_selected_upstream_target_protocol_family(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    candidate: &crate::routing::candidate::RouteCandidate,
) -> Option<String> {
    let family = match candidate.protocol_family.as_str() {
        "openai" => selected_openai_target_family(req, candidate).to_string(),
        "gemini_business" => GEMINI_BUSINESS_IMAGES_FAMILY.to_string(),
        "chataibot" => CHATAIBOT_IMAGES_FAMILY.to_string(),
        "lumalabs" => match req.endpoint_kind {
            EndpointKind::MusicGenerations => LUMALABS_AUDIO_FAMILY.to_string(),
            EndpointKind::VideosGenerations => LUMALABS_VIDEOS_FAMILY.to_string(),
            _ => LUMALABS_IMAGES_FAMILY.to_string(),
        },
        "gemini_canvas" => match req.endpoint_kind {
            EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions
            | EndpointKind::AudioSpeech => GEMINI_GENERATE_CONTENT_FAMILY.to_string(),
            EndpointKind::MusicGenerations => GEMINI_CANVAS_MUSIC_FAMILY.to_string(),
            EndpointKind::VideosGenerations => GEMINI_CANVAS_VIDEOS_FAMILY.to_string(),
            _ => GEMINI_CANVAS_IMAGES_FAMILY.to_string(),
        },
        "producer" => match req.endpoint_kind {
            EndpointKind::ImagesGenerations => PRODUCER_IMAGES_FAMILY.to_string(),
            EndpointKind::VideosGenerations => PRODUCER_VIDEOS_FAMILY.to_string(),
            _ => PRODUCER_MUSIC_FAMILY.to_string(),
        },
        "suno" => match req.endpoint_kind {
            EndpointKind::ImagesGenerations => SUNO_IMAGES_FAMILY.to_string(),
            EndpointKind::VideosGenerations => SUNO_VIDEOS_FAMILY.to_string(),
            _ => SUNO_MUSIC_FAMILY.to_string(),
        },
        "udio" => match req.endpoint_kind {
            EndpointKind::ImagesGenerations => UDIO_IMAGES_FAMILY.to_string(),
            EndpointKind::VideosGenerations => UDIO_VIDEOS_FAMILY.to_string(),
            _ => UDIO_MUSIC_FAMILY.to_string(),
        },
        _ => candidate.protocol_family.clone(),
    };
    Some(family)
}

fn infer_selected_upstream_target_conversation_family(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    candidate: &crate::routing::candidate::RouteCandidate,
) -> Option<String> {
    if !is_conversation_endpoint(req.endpoint_kind) {
        return None;
    }
    match candidate.protocol_family.as_str() {
        "openai" => Some(selected_openai_target_family(req, candidate).to_string()),
        "anthropic" => Some("anthropic_messages".to_string()),
        "gemini_generate_content"
        | "gemini_live"
        | "bedrock_converse"
        | "cohere_chat"
        | "openai_chat"
        | "openai_legacy_completions"
        | "openai_responses"
        | "openai_realtime"
        | "xfyun_websocket" => Some(candidate.protocol_family.clone()),
        _ => Some(candidate.protocol_family.clone()),
    }
}

fn infer_canonical_conversation_semantics(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
) -> Option<&'static str> {
    if !is_conversation_endpoint(req.endpoint_kind) {
        return None;
    }

    match req.protocol_family {
        ProtocolFamily::OpenAi => match req.endpoint_kind {
            EndpointKind::Responses => Some("responses_input_output"),
            EndpointKind::Completions => Some("prompt_completion"),
            _ => Some("messages_turns"),
        },
        ProtocolFamily::OpenAiRealtime | ProtocolFamily::GeminiLive => Some("live_session_events"),
        ProtocolFamily::Anthropic | ProtocolFamily::BedrockConverse => Some("messages_blocks"),
        ProtocolFamily::GeminiGenerateContent => Some("parts_turns"),
        ProtocolFamily::CohereChat => Some("messages_turns"),
        ProtocolFamily::SearchApi => None,
    }
}

fn infer_selected_upstream_target_tool_family(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    candidate: &crate::routing::candidate::RouteCandidate,
    tools_were_injected: bool,
) -> Option<String> {
    if !is_conversation_endpoint(req.endpoint_kind) {
        return None;
    }
    if tools_were_injected {
        return Some("xml_fallback".to_string());
    }

    infer_selected_upstream_target_conversation_family(req, candidate)
}

fn infer_tool_strategy(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    tools_were_injected: bool,
) -> Option<&'static str> {
    if !is_conversation_endpoint(req.endpoint_kind) {
        return None;
    }
    if tools_were_injected {
        return Some("xml_fallback");
    }
    if req.tools.is_empty() {
        return None;
    }
    Some("native")
}

fn infer_canonical_tool_choice_semantics(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    tools_were_injected: bool,
) -> Option<String> {
    if !is_conversation_endpoint(req.endpoint_kind) {
        return None;
    }
    if tools_were_injected {
        return Some(
            CanonicalToolChoice::PromptOnly
                .semantics_label()
                .to_string(),
        );
    }
    if req.tools.is_empty() {
        return None;
    }

    let choice = tool_choice::parse_tool_choice(req.tool_choice.as_ref())
        .unwrap_or(CanonicalToolChoice::Auto);
    Some(choice.semantics_label().to_string())
}

pub(super) fn infer_canonical_completion_semantics(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    resp: &crate::protocol::canonical::CanonicalRelayResponse,
) -> Option<&'static str> {
    if !is_conversation_endpoint(req.endpoint_kind) {
        return None;
    }

    match resp
        .finish_reason
        .as_deref()
        .unwrap_or(if resp.tool_calls.is_empty() {
            "stop"
        } else {
            "tool_calls"
        }) {
        "stop" | "end_turn" | "stop_sequence" | "complete" => Some("stop"),
        "tool_calls" | "function_call" => Some("tool_calls"),
        "length" => Some("length"),
        "content_filter" => Some("content_filter"),
        _ => Some("other_provider_reason"),
    }
}

pub(super) fn set_selected_candidate(
    ctx: &mut PipelineContext,
    candidate: &crate::routing::candidate::RouteCandidate,
    projected_access: Option<&crate::db::ProjectedPlatformAccessRow>,
    model: &str,
    tools_were_injected: bool,
) {
    ctx.selected_provider_id = Some(candidate.provider_account_id.clone());
    ctx.selected_provider_credential_id = candidate.provider_credential_id.clone();
    ctx.selected_provider_label = Some(candidate.label.clone());
    ctx.selected_adapter = Some(candidate.adapter.clone());
    ctx.selected_protocol_profile = Some(candidate.protocol_profile.clone());
    if let Some(line) =
        crate::implementation_lines::line_for_payload(&candidate.payload).or_else(|| {
            crate::implementation_lines::line_for_protocol_profile(&candidate.protocol_profile)
        })
    {
        let line_id = line
            .feature_name()
            .strip_prefix("line-")
            .unwrap_or(line.feature_name());
        crate::http::route_proof::record_route_proof(crate::http::route_proof::RouteProof::new(
            line_id,
        ));
    }
    ctx.resolved_model = Some(model.to_string());
    ctx.selected_model_alias = candidate.model_alias.clone();
    ctx.selected_execution_mode = Some(
        match candidate.resolved_execution_mode {
            crate::routing::candidate::ProviderExecutionMode::DirectHttp => "direct_http",
            crate::routing::candidate::ProviderExecutionMode::BrowserBacked => "browser_backed",
        }
        .to_string(),
    );
    ctx.selected_upstream_target_protocol_family =
        infer_selected_upstream_target_protocol_family(&ctx.canonical_req, candidate);
    ctx.selected_upstream_target_conversation_family =
        infer_selected_upstream_target_conversation_family(&ctx.canonical_req, candidate);
    ctx.canonical_conversation_semantics =
        infer_canonical_conversation_semantics(&ctx.canonical_req).map(str::to_string);
    ctx.selected_upstream_target_tool_family = infer_selected_upstream_target_tool_family(
        &ctx.canonical_req,
        candidate,
        tools_were_injected,
    )
    .map(|value| value.to_string());
    ctx.tool_strategy =
        infer_tool_strategy(&ctx.canonical_req, tools_were_injected).map(str::to_string);
    ctx.canonical_tool_choice_semantics =
        infer_canonical_tool_choice_semantics(&ctx.canonical_req, tools_were_injected);
    ctx.selected_routing_score = candidate.routing_score;
    ctx.selected_health_weight = candidate.routing_health_weight;
    ctx.selected_capacity_weight = candidate.routing_capacity_weight;
    ctx.selected_degraded = candidate.routing_degraded;
    ctx.selected_breaker_open = candidate.routing_breaker_open;
    ctx.selected_degradation_reasons = candidate.routing_degradation_reasons.clone();
    if let Some(projected_access) = projected_access {
        ctx.source_access_key_id = Some(projected_access.source_access_key_id.clone());
        ctx.selected_platform_access_id = Some(projected_access.platform_access_id.clone());
    }
    ctx.selected_real_credential_ref = candidate.payload.credential_id.clone();
}
