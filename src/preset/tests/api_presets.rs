use super::*;

#[test]
fn openai_profile_presets_use_openai_compatible_adapter() {
    for preset in [
        groq_openai_preset(),
        together_openai_preset(),
        openrouter_openai_preset(),
        deepseek_openai_preset(),
        mistral_openai_preset(),
        xai_openai_preset(),
    ] {
        assert_eq!(preset.adapter, "openai_compatible");
        assert!(preset.default_model.is_none());
    }
}

#[test]
fn gemini_and_google_agent_platform_presets_use_expected_auth_headers() {
    let gemini = gemini_api_preset();
    assert_eq!(gemini.adapter, "gemini_api_compatible");
    assert_eq!(gemini.auth_header_name.as_deref(), Some("x-goog-api-key"));
    assert!(gemini.default_model.is_none());

    let aistudio = aistudio_official_api_preset();
    assert_eq!(aistudio.id, "aistudio-official-api");
    assert_eq!(aistudio.adapter, "gemini_api_compatible");
    assert_eq!(aistudio.auth_header_name.as_deref(), Some("x-goog-api-key"));

    let agent_platform = google_agent_platform_preset();
    assert_eq!(agent_platform.id, "google-agent-platform");
    assert_eq!(agent_platform.adapter, "gemini_api_compatible");
    assert_eq!(
        agent_platform.auth_header_name.as_deref(),
        Some("authorization")
    );
    assert!(agent_platform.default_model.is_none());

    let agent_platform_official = google_agent_platform_official_api_preset();
    assert_eq!(
        agent_platform_official.id,
        "google-agent-platform-official-api"
    );
    assert_eq!(agent_platform_official.adapter, "gemini_api_compatible");
    assert_eq!(
        agent_platform_official.auth_header_name.as_deref(),
        Some("authorization")
    );
}

#[test]
fn bedrock_and_cohere_profile_presets_use_native_adapters() {
    let bedrock = bedrock_converse_preset();
    assert_eq!(bedrock.adapter, "bedrock_converse_compatible");
    assert_eq!(bedrock.auth_header_name.as_deref(), Some("authorization"));
    assert!(bedrock.default_model.is_none());

    let cohere = cohere_chat_preset();
    assert_eq!(cohere.adapter, "cohere_compatible");
    assert!(cohere.default_model.is_none());
}

#[test]
fn xfyun_preset_uses_v2_chat_completions_path() {
    let preset = xfyun_preset();
    assert_eq!(preset.adapter, "openai_compatible");
    assert_eq!(
        preset.chat_completions_path.as_deref(),
        Some("/v2/chat/completions")
    );
    assert!(preset.responses_path.is_none());
    assert!(preset.default_model.is_none());
}

#[test]
fn xfyun_websocket_preset_uses_native_ws_path() {
    let preset = xfyun_websocket_preset();
    assert_eq!(preset.adapter, "xfyun_websocket_compatible");
    assert_eq!(preset.chat_completions_path.as_deref(), Some("/v1.1/chat"));
    assert_eq!(
        preset.extra_body.get("uid"),
        Some(&Value::String("gateway".to_string()))
    );
    assert!(preset.responses_path.is_none());
}

#[test]
fn groq_and_together_presets_use_nonduplicating_chat_paths() {
    let groq = groq_openai_preset();
    assert_eq!(groq.adapter, "openai_compatible");
    assert_eq!(
        groq.chat_completions_path.as_deref(),
        Some("/chat/completions")
    );
    assert!(groq.responses_path.is_none());

    let together = together_openai_preset();
    assert_eq!(together.adapter, "openai_compatible");
    assert_eq!(
        together.chat_completions_path.as_deref(),
        Some("/chat/completions")
    );
    assert!(together.responses_path.is_none());
}

#[test]
fn perplexity_preset_uses_official_openai_compatible_paths() {
    let preset = perplexity_preset();
    assert_eq!(preset.adapter, "openai_compatible");
    assert_eq!(preset.default_model.as_deref(), Some("sonar"));
    assert_eq!(
        preset.chat_completions_path.as_deref(),
        Some("/chat/completions")
    );
    assert_eq!(preset.responses_path.as_deref(), Some("/v1/responses"));
}

#[test]
fn nvidia_openai_preset_uses_openai_compatible_adapter() {
    let preset = nvidia_openai_preset();
    assert_eq!(preset.id, "nvidia-openai");
    assert_eq!(preset.adapter, "openai_compatible");
    assert_eq!(
        preset.chat_completions_path.as_deref(),
        Some("/v1/chat/completions")
    );
    assert!(preset.responses_path.is_none());
    assert!(preset.default_model.is_none());
}

#[test]
fn qwen_legacy_preset_points_to_dashscope_openai_line() {
    let preset = qwen_preset();
    assert_eq!(preset.id, "qwen");
    assert_eq!(preset.adapter, "openai_compatible");
    assert_eq!(preset.auth_mode.as_deref(), Some("bearer"));
    assert!(preset.session_auth.is_none());
    assert!(preset.headers.is_empty());
}

#[test]
fn qwen_official_api_presets_use_current_protocols() {
    let dashscope = qwen_dashscope_openai_preset();
    assert_eq!(dashscope.adapter, "openai_compatible");
    assert_eq!(dashscope.auth_mode.as_deref(), Some("bearer"));

    let coding_openai = qwen_coding_plan_openai_preset();
    assert_eq!(coding_openai.adapter, "openai_compatible");
    assert_eq!(coding_openai.auth_mode.as_deref(), Some("bearer"));

    let coding_anthropic = qwen_coding_plan_anthropic_preset();
    assert_eq!(coding_anthropic.adapter, "anthropic_compatible");
    assert_eq!(coding_anthropic.auth_mode.as_deref(), Some("x-api-key"));
    assert_eq!(
        coding_anthropic.anthropic_version.as_deref(),
        Some("2023-06-01")
    );
}
