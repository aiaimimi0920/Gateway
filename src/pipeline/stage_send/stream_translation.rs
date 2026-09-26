//! Lazy usage observation and protocol/tool stream normalization in original order.
use super::*;

pub(super) fn translate(
    ctx: &PipelineContext,
    candidate: &crate::routing::candidate::RouteCandidate,
    attempt: &PreparedAttempt,
    byte_stream: ByteStream,
) -> (ByteStream, Option<UsageHandle>) {
    let PreparedAttempt {
        reply_model,
        original_tools,
        original_tool_choice,
        original_messages_text,
        tools_were_injected,
        ..
    } = attempt;
    let tools_were_injected = *tools_were_injected;
    let (byte_stream, stream_usage_handle) = if candidate.adapter == "kiro_compatible" {
        (byte_stream, None)
    } else {
        let (tapped_stream, usage_handle) = tap_sse_usage(byte_stream);
        (
            Box::pin(tapped_stream)
                as std::pin::Pin<
                    Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>,
                >,
            Some(usage_handle),
        )
    };

    let uses_openai_responses_bridge = candidate
        .payload
        .bridges_openai_text_endpoint_to_responses(ctx.canonical_req.endpoint_kind);
    let tool_detection_placement = tool_detection_placement(
        ctx.canonical_req.endpoint_kind,
        &candidate.adapter,
        uses_openai_responses_bridge,
        tools_were_injected,
    );

    // Detect directly only when the upstream already speaks
    // OpenAI chat SSE. Other paths detect after normalization.
    let byte_stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>,
    > = if tool_detection_placement.on_raw_openai() {
        wrap_injected_openai_stream(
            byte_stream,
            true,
            &ctx.req_id,
            &reply_model,
            original_tools.clone(),
            original_tool_choice.clone(),
            Some(original_messages_text.clone()),
        )
    } else {
        byte_stream
    };

    // If the client called /v1/messages (Anthropic native) but
    // the upstream is NOT anthropic_compatible, the stream is in
    // OpenAI SSE format and must be translated to Anthropic SSE.
    let byte_stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>,
    > = if candidate.adapter == "kiro_compatible" {
        if ctx.canonical_req.endpoint_kind == EndpointKind::Messages {
            debug!(
                req_id = %ctx.req_id,
                "translating Kiro event stream to Anthropic SSE format"
            );
            Box::pin(kiro::translate_kiro_event_stream_to_anthropic_sse(
                byte_stream,
                reply_model.clone(),
                ctx.canonical_req.clone(),
            ))
        } else if ctx.canonical_req.endpoint_kind == EndpointKind::Responses {
            debug!(
                req_id = %ctx.req_id,
                "translating Kiro event stream to OpenAI Responses SSE format"
            );
            Box::pin(responses::translate_openai_sse_to_responses(
                kiro::translate_kiro_event_stream_to_openai_sse(
                    byte_stream,
                    reply_model.clone(),
                    ctx.canonical_req.clone(),
                ),
                reply_model.clone(),
            ))
        } else {
            debug!(
                req_id = %ctx.req_id,
                "translating Kiro event stream to OpenAI SSE format"
            );
            Box::pin(kiro::translate_kiro_event_stream_to_openai_sse(
                byte_stream,
                reply_model.clone(),
                ctx.canonical_req.clone(),
            ))
        }
    } else if ctx.canonical_req.endpoint_kind == EndpointKind::ChatCompletions
        && uses_openai_responses_bridge
    {
        debug!(
            req_id = %ctx.req_id,
            adapter = %candidate.adapter,
            "translating OpenAI Responses SSE stream to OpenAI chat SSE format"
        );
        let openai_stream =
            responses::translate_responses_sse_to_openai_chat(byte_stream, reply_model.clone());
        wrap_injected_openai_stream(
            Box::pin(openai_stream),
            tool_detection_placement.after_normalization(),
            &ctx.req_id,
            &reply_model,
            original_tools.clone(),
            original_tool_choice.clone(),
            Some(original_messages_text.clone()),
        )
    } else if ctx.canonical_req.endpoint_kind == EndpointKind::ChatCompletions
        && candidate.adapter == "anthropic_compatible"
    {
        debug!(
            req_id = %ctx.req_id,
            adapter = %candidate.adapter,
            "translating Anthropic SSE stream to OpenAI SSE format"
        );
        let openai_stream =
            accio::translate_anthropic_like_stream_to_openai(byte_stream, reply_model.clone());
        wrap_injected_openai_stream(
            Box::pin(openai_stream),
            tool_detection_placement.after_normalization(),
            &ctx.req_id,
            &reply_model,
            original_tools.clone(),
            original_tool_choice.clone(),
            Some(original_messages_text.clone()),
        )
    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Responses
        && candidate.adapter == "anthropic_compatible"
    {
        debug!(
            req_id = %ctx.req_id,
            adapter = %candidate.adapter,
            "translating Anthropic SSE stream to OpenAI Responses SSE format"
        );
        let openai_stream =
            accio::translate_anthropic_like_stream_to_openai(byte_stream, reply_model.clone());
        let openai_stream = wrap_injected_openai_stream(
            Box::pin(openai_stream),
            tool_detection_placement.after_normalization(),
            &ctx.req_id,
            &reply_model,
            original_tools.clone(),
            original_tool_choice.clone(),
            Some(original_messages_text.clone()),
        );
        Box::pin(responses::translate_openai_sse_to_responses(
            openai_stream,
            reply_model.clone(),
        ))
    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Messages
        && uses_openai_responses_bridge
    {
        debug!(
            req_id = %ctx.req_id,
            adapter = %candidate.adapter,
            "translating OpenAI Responses SSE stream to Anthropic SSE format"
        );
        let openai_stream =
            responses::translate_responses_sse_to_openai_chat(byte_stream, reply_model.clone());
        let openai_stream = wrap_injected_openai_stream(
            Box::pin(openai_stream),
            tool_detection_placement.after_normalization(),
            &ctx.req_id,
            &reply_model,
            original_tools.clone(),
            original_tool_choice.clone(),
            Some(original_messages_text.clone()),
        );
        Box::pin(anthropic::translate_openai_sse_to_anthropic(
            openai_stream,
            reply_model.clone(),
        ))
    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Messages
        && candidate.adapter != "anthropic_compatible"
    {
        debug!(
            req_id = %ctx.req_id,
            adapter = %candidate.adapter,
            "translating OpenAI SSE stream to Anthropic SSE format"
        );
        let openai_stream = wrap_injected_openai_stream(
            byte_stream,
            tool_detection_placement.after_normalization(),
            &ctx.req_id,
            &reply_model,
            original_tools.clone(),
            original_tool_choice.clone(),
            Some(original_messages_text.clone()),
        );
        Box::pin(anthropic::translate_openai_sse_to_anthropic(
            openai_stream,
            reply_model.clone(),
        ))
    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Completions
        && uses_openai_responses_bridge
    {
        debug!(
            req_id = %ctx.req_id,
            adapter = %candidate.adapter,
            "translating OpenAI Responses SSE stream to legacy completions SSE format"
        );
        let openai_stream =
            responses::translate_responses_sse_to_openai_chat(byte_stream, reply_model.clone());
        let openai_stream = wrap_injected_openai_stream(
            Box::pin(openai_stream),
            tool_detection_placement.after_normalization(),
            &ctx.req_id,
            &reply_model,
            original_tools.clone(),
            original_tool_choice.clone(),
            Some(original_messages_text.clone()),
        );
        Box::pin(openai::translate_openai_chat_sse_to_legacy_completions(
            openai_stream,
            reply_model.clone(),
        ))
    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Completions
        && candidate.adapter == "anthropic_compatible"
    {
        debug!(
            req_id = %ctx.req_id,
            adapter = %candidate.adapter,
            "translating Anthropic SSE stream to legacy completions SSE format"
        );
        Box::pin(openai::translate_openai_chat_sse_to_legacy_completions(
            accio::translate_anthropic_like_stream_to_openai(byte_stream, reply_model.clone()),
            reply_model.clone(),
        ))
    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Completions {
        debug!(
            req_id = %ctx.req_id,
            adapter = %candidate.adapter,
            "translating OpenAI chat SSE stream to legacy completions SSE format"
        );
        Box::pin(openai::translate_openai_chat_sse_to_legacy_completions(
            byte_stream,
            reply_model.clone(),
        ))
    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Responses {
        debug!(
            req_id = %ctx.req_id,
            adapter = %candidate.adapter,
            "translating stream to OpenAI Responses SSE format"
        );
        let openai_stream = if uses_openai_responses_bridge || tools_were_injected {
            Box::pin(responses::translate_responses_sse_to_openai_chat(
                byte_stream,
                reply_model.clone(),
            ))
                as std::pin::Pin<
                    Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>,
                >
        } else {
            byte_stream
        };
        let openai_stream = wrap_injected_openai_stream(
            openai_stream,
            tool_detection_placement.after_normalization(),
            &ctx.req_id,
            &reply_model,
            original_tools.clone(),
            original_tool_choice.clone(),
            Some(original_messages_text.clone()),
        );
        Box::pin(responses::translate_openai_sse_to_responses(
            openai_stream,
            reply_model.clone(),
        ))
    } else {
        byte_stream
    };
    (byte_stream, stream_usage_handle)
}
