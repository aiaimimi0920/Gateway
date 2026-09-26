use super::*;

#[test]
fn selected_candidate_tracks_openai_responses_bridge_semantics() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
    let mut ctx = crate::pipeline::PipelineContext::new(req, None);
    let mut candidate = make_candidate("openai_compatible");
    candidate.payload.responses_path = Some("/v1/responses".to_string());

    set_selected_candidate(&mut ctx, &candidate, None, "gpt-4o", false);

    assert_eq!(
        ctx.selected_upstream_target_protocol_family.as_deref(),
        Some("openai_responses")
    );
    assert_eq!(
        ctx.selected_upstream_target_conversation_family.as_deref(),
        Some("openai_responses")
    );
    assert_eq!(
        ctx.canonical_conversation_semantics.as_deref(),
        Some("messages_turns")
    );
}

#[test]
fn selected_candidate_tracks_native_modality_family() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::MusicGenerations);
    let mut ctx = crate::pipeline::PipelineContext::new(req, None);
    let mut candidate = make_candidate("producer_compatible");
    candidate.protocol_family = PRODUCER_MUSIC_FAMILY.to_string();

    set_selected_candidate(&mut ctx, &candidate, None, "producer-base", false);

    assert_eq!(
        ctx.selected_upstream_target_protocol_family.as_deref(),
        Some(PRODUCER_MUSIC_FAMILY)
    );
    assert!(ctx.selected_upstream_target_conversation_family.is_none());
    assert!(ctx.selected_upstream_target_tool_family.is_none());
}

#[test]
fn selected_candidate_tracks_producer_image_family() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    let mut ctx = crate::pipeline::PipelineContext::new(req, None);
    let mut candidate = make_candidate("producer_compatible");
    candidate.protocol_family = "producer".to_string();
    candidate.protocol_profile = "producer".to_string();

    set_selected_candidate(&mut ctx, &candidate, None, "producer:image", false);

    assert_eq!(
        ctx.selected_upstream_target_protocol_family.as_deref(),
        Some(PRODUCER_IMAGES_FAMILY)
    );
    assert!(ctx.selected_upstream_target_conversation_family.is_none());
}

#[test]
fn selected_candidate_tracks_lumalabs_video_family() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
    let mut ctx = crate::pipeline::PipelineContext::new(req, None);
    let mut candidate = make_candidate("lumalabs_compatible");
    candidate.protocol_family = "lumalabs".to_string();
    candidate.protocol_profile = "lumalabs".to_string();

    set_selected_candidate(&mut ctx, &candidate, None, "ray-2", false);

    assert_eq!(
        ctx.selected_upstream_target_protocol_family.as_deref(),
        Some(LUMALABS_VIDEOS_FAMILY)
    );
    assert!(ctx.selected_upstream_target_conversation_family.is_none());
}

#[test]
fn selected_candidate_tracks_suno_video_family() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
    let mut ctx = crate::pipeline::PipelineContext::new(req, None);
    let mut candidate = make_candidate("suno_compatible");
    candidate.protocol_family = "suno".to_string();
    candidate.protocol_profile = "suno".to_string();

    set_selected_candidate(&mut ctx, &candidate, None, "chirp-v3-5", false);

    assert_eq!(
        ctx.selected_upstream_target_protocol_family.as_deref(),
        Some(SUNO_VIDEOS_FAMILY)
    );
    assert!(ctx.selected_upstream_target_conversation_family.is_none());
}

#[test]
fn selected_candidate_tracks_udio_video_family() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
    let mut ctx = crate::pipeline::PipelineContext::new(req, None);
    let mut candidate = make_candidate("udio_compatible");
    candidate.protocol_family = "udio".to_string();
    candidate.protocol_profile = "udio".to_string();

    set_selected_candidate(&mut ctx, &candidate, None, "udio-video", false);

    assert_eq!(
        ctx.selected_upstream_target_protocol_family.as_deref(),
        Some(UDIO_VIDEOS_FAMILY)
    );
    assert!(ctx.selected_upstream_target_conversation_family.is_none());
}

#[test]
fn selected_candidate_tracks_prompt_only_when_tools_are_injected() {
    let mut req = make_req(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
    req.tools.push(CanonicalTool {
        tool_type: "function".to_string(),
        name: Some("weather".to_string()),
        description: None,
        input_schema: Some(serde_json::json!({"type":"object"})),
        raw: HashMap::new(),
    });
    req.tool_choice = Some(serde_json::json!("required"));
    let mut ctx = crate::pipeline::PipelineContext::new(req, None);
    let candidate = make_candidate("openai_compatible");

    set_selected_candidate(&mut ctx, &candidate, None, "gpt-4o", true);

    assert_eq!(
        ctx.selected_upstream_target_tool_family.as_deref(),
        Some("xml_fallback")
    );
    assert_eq!(ctx.tool_strategy.as_deref(), Some("xml_fallback"));
    assert_eq!(
        ctx.canonical_tool_choice_semantics.as_deref(),
        Some("prompt_only")
    );
}

#[test]
fn xml_tool_response_bridge_promotes_upstream_xml_when_tools_requested() {
    let mut canonical_resp = CanonicalRelayResponse {
            model: "xop35qwen2b".to_string(),
            text: "<tool_calls>\n<tool_call>\n<tool_name>weather</tool_name>\n<parameters>{\"city\":\"Hangzhou\"}</parameters>\n</tool_call>\n</tool_calls>".to_string(),
            usage: None,
            tool_calls: Vec::new(),
            upstream_status: Some(200),
            finish_reason: Some("stop".to_string()),
        };
    let tools = vec![CanonicalTool {
        tool_type: "function".to_string(),
        name: Some("weather".to_string()),
        description: Some("Return weather".to_string()),
        input_schema: Some(serde_json::json!({
            "type": "object",
            "properties": { "city": { "type": "string" } },
            "required": ["city"],
        })),
        raw: HashMap::new(),
    }];

    let changed = maybe_apply_xml_tool_response_bridge(
        &uuid::Uuid::nil(),
        &mut canonical_resp,
        &tools,
        Some(&serde_json::json!("required")),
        Some("Use only the weather tool for Hangzhou."),
        false,
    );

    assert!(changed);
    assert!(canonical_resp.text.is_empty());
    assert_eq!(canonical_resp.tool_calls.len(), 1);
    assert_eq!(
        canonical_resp.tool_calls[0].name.as_deref(),
        Some("weather")
    );
    assert_eq!(canonical_resp.finish_reason.as_deref(), Some("tool_calls"));
}

#[test]
fn xml_tool_response_bridge_skips_when_native_tool_calls_already_present() {
    let mut canonical_resp = CanonicalRelayResponse {
        model: "xop35qwen2b".to_string(),
        text: "<tool_calls><tool_call><tool_name>weather</tool_name></tool_call></tool_calls>"
            .to_string(),
        usage: None,
        tool_calls: vec![crate::protocol::canonical::CanonicalToolCall {
            id: Some("call_weather".to_string()),
            call_type: "function".to_string(),
            name: Some("weather".to_string()),
            arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
            raw: HashMap::new(),
        }],
        upstream_status: Some(200),
        finish_reason: Some("tool_calls".to_string()),
    };
    let tools = vec![CanonicalTool {
        tool_type: "function".to_string(),
        name: Some("weather".to_string()),
        description: None,
        input_schema: Some(serde_json::json!({"type": "object"})),
        raw: HashMap::new(),
    }];

    let changed = maybe_apply_xml_tool_response_bridge(
        &uuid::Uuid::nil(),
        &mut canonical_resp,
        &tools,
        None,
        None,
        true,
    );

    assert!(!changed);
    assert_eq!(canonical_resp.tool_calls.len(), 1);
}

#[test]
fn canonical_completion_semantics_maps_content_filter() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
    let resp = CanonicalRelayResponse {
        finish_reason: Some("content_filter".to_string()),
        ..make_resp()
    };

    assert_eq!(
        infer_canonical_completion_semantics(&req, &resp),
        Some("content_filter")
    );
}
