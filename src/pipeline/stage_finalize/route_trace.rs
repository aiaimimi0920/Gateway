//! Stable route-trace JSON and explicit protocol/endpoint labels.

use super::*;

pub(super) fn build_route_trace_from_ctx(
    ctx: &PipelineContext,
    error: Option<&crate::error::GatewayError>,
) -> Value {
    let attempted_provider_ids = ctx.attempted_provider_ids();

    let failure_classification = error.map(|value| {
        classify_provider_failure(
            value.http_status,
            value.code.as_deref(),
            Some(value.message.as_str()),
        )
    });

    json!({
        "requestedProtocolFamily": protocol_family_name(ctx),
        "endpointKind": endpoint_kind_name(ctx),
        "routeAttemptCount": ctx.route_attempt_count(),
        "candidateCount": ctx.candidates.len(),
        "attemptedProviderIds": attempted_provider_ids,
        "selectedPipelineMode": ctx.selected_execution_mode.clone(),
        "selectedProtocolProfile": ctx.selected_protocol_profile.clone(),
        "selectionStrategy": ctx.route_selection_strategy.clone(),
        "routeSelectionStrategy": ctx.route_selection_strategy.clone(),
        "accessKeyId": ctx.requesting_access_key_id.clone(),
        "sourceAccessKeyId": ctx.source_access_key_id.clone(),
        "platformAccessId": ctx.selected_platform_access_id.clone(),
        "realCredentialRef": ctx.selected_real_credential_ref.clone(),
        "selectedUpstreamTargetProtocolFamily": ctx
            .selected_upstream_target_protocol_family
            .clone(),
        "selectedUpstreamTargetConversationFamily": ctx
            .selected_upstream_target_conversation_family
            .clone(),
        "canonicalConversationSemantics": ctx.canonical_conversation_semantics.clone(),
        "selectedUpstreamTargetToolFamily": ctx.selected_upstream_target_tool_family.clone(),
        "toolStrategy": ctx.tool_strategy.clone(),
        "canonicalToolChoiceSemantics": ctx.canonical_tool_choice_semantics.clone(),
        "canonicalCompletionSemantics": ctx.canonical_completion_semantics.clone(),
        "selectedCandidate": build_selected_candidate_trace_from_ctx(ctx),
        "fallbackEligible": error.map(is_fallback_eligible),
        "errorCode": error.and_then(|value| value.code.clone()),
        "errorKind": error.map(|value| format!("{:?}", value.kind)),
        "failureClass": failure_classification.as_ref().map(|value| value.class_name()),
        "failureScope": failure_classification.as_ref().map(|value| value.scope_name()),
        "failurePermanent": failure_classification.as_ref().map(|value| value.permanent),
    })
}

pub(super) fn build_route_trace_from_snapshot(
    snapshot: &RequestAuditFinalizeSnapshot,
    error_summary: Option<&str>,
) -> Value {
    let failure_classification = error_summary
        .map(|summary| classify_provider_failure(None, Some("stream_failed"), Some(summary)));
    json!({
        "requestedProtocolFamily": snapshot.protocol_family.clone(),
        "endpointKind": snapshot.endpoint_kind.clone(),
        "routeAttemptCount": snapshot.route_attempt_count,
        "attemptedProviderIds": snapshot.attempted_provider_ids.clone(),
        "selectedPipelineMode": snapshot.selected_execution_mode.clone(),
        "selectedProtocolProfile": snapshot.selected_protocol_profile.clone(),
        "selectionStrategy": snapshot.route_selection_strategy.clone(),
        "routeSelectionStrategy": snapshot.route_selection_strategy.clone(),
        "accessKeyId": snapshot.access_key_id.clone(),
        "sourceAccessKeyId": snapshot.source_access_key_id.clone(),
        "platformAccessId": snapshot.platform_access_id.clone(),
        "realCredentialRef": snapshot.real_credential_ref.clone(),
        "selectedUpstreamTargetProtocolFamily": snapshot
            .selected_upstream_target_protocol_family
            .clone(),
        "selectedUpstreamTargetConversationFamily": snapshot
            .selected_upstream_target_conversation_family
            .clone(),
        "canonicalConversationSemantics": snapshot.canonical_conversation_semantics.clone(),
        "selectedUpstreamTargetToolFamily": snapshot
            .selected_upstream_target_tool_family
            .clone(),
        "toolStrategy": snapshot.tool_strategy.clone(),
        "canonicalToolChoiceSemantics": snapshot.canonical_tool_choice_semantics.clone(),
        "canonicalCompletionSemantics": snapshot.canonical_completion_semantics.clone(),
        "selectedCandidate": build_selected_candidate_trace_from_snapshot(snapshot),
        "errorSummary": error_summary,
        "failureClass": failure_classification.as_ref().map(|value| value.class_name()),
        "failureScope": failure_classification.as_ref().map(|value| value.scope_name()),
        "failurePermanent": failure_classification.as_ref().map(|value| value.permanent),
    })
}

fn build_selected_candidate_trace_from_ctx(ctx: &PipelineContext) -> Option<Value> {
    let provider_account_id = ctx.selected_provider_id.as_ref()?;
    let mut trace = serde_json::Map::new();
    trace.insert("providerAccountId".to_string(), json!(provider_account_id));
    trace.insert(
        "platformAccessId".to_string(),
        json!(ctx.selected_platform_access_id.clone()),
    );
    trace.insert(
        "sourceAccessKeyId".to_string(),
        json!(ctx.source_access_key_id.clone()),
    );
    trace.insert(
        "realCredentialRef".to_string(),
        json!(ctx.selected_real_credential_ref.clone()),
    );
    trace.insert(
        "providerLabel".to_string(),
        json!(ctx.selected_provider_label.clone()),
    );
    trace.insert(
        "label".to_string(),
        json!(ctx.selected_provider_label.clone()),
    );
    trace.insert("adapter".to_string(), json!(ctx.selected_adapter.clone()));
    trace.insert(
        "protocolProfile".to_string(),
        json!(ctx.selected_protocol_profile.clone()),
    );
    trace.insert(
        "modelAlias".to_string(),
        json!(ctx.selected_model_alias.clone()),
    );
    trace.insert(
        "resolvedModel".to_string(),
        json!(ctx.resolved_model.clone()),
    );
    trace.insert(
        "resolvedExecutionMode".to_string(),
        json!(ctx.selected_execution_mode.clone()),
    );
    trace.insert(
        "executionMode".to_string(),
        json!(ctx.selected_execution_mode.clone()),
    );
    trace.insert(
        "selectedUpstreamTargetProtocolFamily".to_string(),
        json!(ctx.selected_upstream_target_protocol_family.clone()),
    );
    trace.insert(
        "selectedUpstreamTargetConversationFamily".to_string(),
        json!(ctx.selected_upstream_target_conversation_family.clone()),
    );
    trace.insert(
        "canonicalConversationSemantics".to_string(),
        json!(ctx.canonical_conversation_semantics.clone()),
    );
    trace.insert(
        "selectedUpstreamTargetToolFamily".to_string(),
        json!(ctx.selected_upstream_target_tool_family.clone()),
    );
    trace.insert("toolStrategy".to_string(), json!(ctx.tool_strategy.clone()));
    trace.insert(
        "canonicalToolChoiceSemantics".to_string(),
        json!(ctx.canonical_tool_choice_semantics.clone()),
    );
    trace.insert(
        "canonicalCompletionSemantics".to_string(),
        json!(ctx.canonical_completion_semantics.clone()),
    );
    trace.insert(
        "routingScore".to_string(),
        json!(ctx.selected_routing_score),
    );
    trace.insert(
        "healthWeight".to_string(),
        json!(ctx.selected_health_weight),
    );
    trace.insert(
        "capacityWeight".to_string(),
        json!(ctx.selected_capacity_weight),
    );
    trace.insert("degraded".to_string(), json!(ctx.selected_degraded));
    trace.insert("breakerOpen".to_string(), json!(ctx.selected_breaker_open));
    if !ctx.selected_degradation_reasons.is_empty() {
        trace.insert(
            "degradationReasons".to_string(),
            json!(ctx.selected_degradation_reasons.clone()),
        );
    }
    Some(Value::Object(trace))
}

fn build_selected_candidate_trace_from_snapshot(
    snapshot: &RequestAuditFinalizeSnapshot,
) -> Option<Value> {
    let provider_account_id = snapshot.provider_account_id.as_ref()?;
    let mut trace = serde_json::Map::new();
    trace.insert("providerAccountId".to_string(), json!(provider_account_id));
    trace.insert(
        "platformAccessId".to_string(),
        json!(snapshot.platform_access_id.clone()),
    );
    trace.insert(
        "sourceAccessKeyId".to_string(),
        json!(snapshot.source_access_key_id.clone()),
    );
    trace.insert(
        "realCredentialRef".to_string(),
        json!(snapshot.real_credential_ref.clone()),
    );
    trace.insert(
        "providerLabel".to_string(),
        json!(snapshot.selected_provider_label.clone()),
    );
    trace.insert(
        "label".to_string(),
        json!(snapshot.selected_provider_label.clone()),
    );
    trace.insert(
        "adapter".to_string(),
        json!(snapshot.selected_adapter.clone()),
    );
    trace.insert(
        "modelAlias".to_string(),
        json!(snapshot.model_alias.clone()),
    );
    trace.insert(
        "resolvedModel".to_string(),
        json!(snapshot.resolved_model.clone()),
    );
    trace.insert(
        "resolvedExecutionMode".to_string(),
        json!(snapshot.selected_execution_mode.clone()),
    );
    trace.insert(
        "executionMode".to_string(),
        json!(snapshot.selected_execution_mode.clone()),
    );
    trace.insert(
        "protocolProfile".to_string(),
        json!(snapshot.selected_protocol_profile.clone()),
    );
    trace.insert(
        "selectedUpstreamTargetProtocolFamily".to_string(),
        json!(snapshot.selected_upstream_target_protocol_family.clone()),
    );
    trace.insert(
        "selectedUpstreamTargetConversationFamily".to_string(),
        json!(snapshot
            .selected_upstream_target_conversation_family
            .clone()),
    );
    trace.insert(
        "canonicalConversationSemantics".to_string(),
        json!(snapshot.canonical_conversation_semantics.clone()),
    );
    trace.insert(
        "selectedUpstreamTargetToolFamily".to_string(),
        json!(snapshot.selected_upstream_target_tool_family.clone()),
    );
    trace.insert(
        "toolStrategy".to_string(),
        json!(snapshot.tool_strategy.clone()),
    );
    trace.insert(
        "canonicalToolChoiceSemantics".to_string(),
        json!(snapshot.canonical_tool_choice_semantics.clone()),
    );
    trace.insert(
        "canonicalCompletionSemantics".to_string(),
        json!(snapshot.canonical_completion_semantics.clone()),
    );
    trace.insert(
        "routingScore".to_string(),
        json!(snapshot.selected_routing_score),
    );
    trace.insert(
        "healthWeight".to_string(),
        json!(snapshot.selected_health_weight),
    );
    trace.insert(
        "capacityWeight".to_string(),
        json!(snapshot.selected_capacity_weight),
    );
    trace.insert("degraded".to_string(), json!(snapshot.selected_degraded));
    trace.insert(
        "breakerOpen".to_string(),
        json!(snapshot.selected_breaker_open),
    );
    if !snapshot.selected_degradation_reasons.is_empty() {
        trace.insert(
            "degradationReasons".to_string(),
            json!(snapshot.selected_degradation_reasons.clone()),
        );
    }
    Some(Value::Object(trace))
}

fn is_fallback_eligible(error: &crate::error::GatewayError) -> bool {
    matches!(
        &error.fallback_hint,
        crate::error::FallbackHint::FallbackProvider { .. }
            | crate::error::FallbackHint::Retry { .. }
    )
}

pub(super) fn protocol_family_name(ctx: &PipelineContext) -> String {
    match ctx.canonical_req.protocol_family {
        crate::protocol::canonical::ProtocolFamily::OpenAi => match ctx.canonical_req.endpoint_kind
        {
            crate::protocol::canonical::EndpointKind::Responses => OPENAI_RESPONSES_FAMILY,
            crate::protocol::canonical::EndpointKind::Completions => {
                OPENAI_LEGACY_COMPLETIONS_FAMILY
            }
            crate::protocol::canonical::EndpointKind::Embeddings => OPENAI_EMBEDDINGS_FAMILY,
            crate::protocol::canonical::EndpointKind::ImagesGenerations => {
                OPENAI_IMAGES_GENERATIONS_FAMILY
            }
            crate::protocol::canonical::EndpointKind::ImagesEdits => OPENAI_IMAGES_EDITS_FAMILY,
            crate::protocol::canonical::EndpointKind::MusicGenerations => {
                OPENAI_MUSIC_GENERATIONS_FAMILY
            }
            crate::protocol::canonical::EndpointKind::VideosGenerations => {
                OPENAI_VIDEOS_GENERATIONS_FAMILY
            }
            crate::protocol::canonical::EndpointKind::AudioTranscriptions => {
                OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY
            }
            crate::protocol::canonical::EndpointKind::AudioSpeech => OPENAI_AUDIO_SPEECH_FAMILY,
            _ => OPENAI_CHAT_FAMILY,
        },
        crate::protocol::canonical::ProtocolFamily::OpenAiRealtime => OPENAI_REALTIME_FAMILY,
        crate::protocol::canonical::ProtocolFamily::Anthropic => ANTHROPIC_MESSAGES_FAMILY,
        crate::protocol::canonical::ProtocolFamily::GeminiGenerateContent => {
            "gemini_generate_content"
        }
        crate::protocol::canonical::ProtocolFamily::GeminiLive => "gemini_live",
        crate::protocol::canonical::ProtocolFamily::BedrockConverse => "bedrock_converse",
        crate::protocol::canonical::ProtocolFamily::CohereChat => "cohere_chat",
        crate::protocol::canonical::ProtocolFamily::SearchApi => SEARCH_API_FAMILY,
    }
    .to_string()
}

pub(super) fn endpoint_kind_name(ctx: &PipelineContext) -> String {
    match ctx.canonical_req.endpoint_kind {
        crate::protocol::canonical::EndpointKind::ChatCompletions => "chat_completions",
        crate::protocol::canonical::EndpointKind::Completions => "completions",
        crate::protocol::canonical::EndpointKind::Embeddings => "embeddings",
        crate::protocol::canonical::EndpointKind::ImagesGenerations => "images_generations",
        crate::protocol::canonical::EndpointKind::ImagesEdits => "images_edits",
        crate::protocol::canonical::EndpointKind::MusicGenerations => "music_generations",
        crate::protocol::canonical::EndpointKind::VideosGenerations => "videos_generations",
        crate::protocol::canonical::EndpointKind::AudioTranscriptions => "audio_transcriptions",
        crate::protocol::canonical::EndpointKind::AudioSpeech => "audio_speech",
        crate::protocol::canonical::EndpointKind::Messages => "messages",
        crate::protocol::canonical::EndpointKind::Responses => "responses",
        crate::protocol::canonical::EndpointKind::Search => "search",
        crate::protocol::canonical::EndpointKind::Fetch => "fetch",
        crate::protocol::canonical::EndpointKind::ResearchCreate => "research_create",
        crate::protocol::canonical::EndpointKind::ResearchList => "research_list",
        crate::protocol::canonical::EndpointKind::ResearchGet => "research_get",
        crate::protocol::canonical::EndpointKind::CreditsBalance => "credits_balance",
    }
    .to_string()
}

pub(super) fn extract_response_id(body: &Value) -> Option<String> {
    body.get("id").and_then(Value::as_str).map(str::to_string)
}
