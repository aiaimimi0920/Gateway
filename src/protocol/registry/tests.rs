use super::*;

#[test]
fn infer_protocol_profile_prefers_explicit_provider_hint() {
    assert_eq!(
        infer_protocol_profile(
            "openai_compatible",
            Some("groq"),
            Some("https://api.openai.com/v1"),
        ),
        "groq"
    );
}

#[test]
fn infer_protocol_profile_uses_base_url_for_azure_and_perplexity() {
    assert_eq!(
        infer_protocol_profile(
            "openai_compatible",
            None,
            Some("https://example.openai.azure.com/openai/v1"),
        ),
        "azure_openai"
    );
    assert_eq!(
        infer_protocol_profile(
            "search_api_compatible",
            None,
            Some("https://api.perplexity.ai"),
        ),
        "perplexity_search"
    );
}

#[test]
fn infer_protocol_profile_supports_qwen_hint_and_official_urls() {
    assert_eq!(
        infer_protocol_profile(
            "openai_compatible",
            Some("qwen"),
            Some("https://api.openai.com/v1"),
        ),
        "qwen_dashscope_openai"
    );
    assert_eq!(
        infer_protocol_profile(
            "openai_compatible",
            None,
            Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
        ),
        "qwen_dashscope_openai"
    );
    assert_eq!(
        infer_protocol_profile(
            "qwen_web_compatible",
            None,
            Some("https://chat.qwen.ai/api/v1"),
        ),
        "qwen_web_chat"
    );
    assert_eq!(
        infer_protocol_profile(
            "anthropic_compatible",
            None,
            Some("https://coding.dashscope.aliyuncs.com/apps/anthropic"),
        ),
        "qwen_coding_plan_anthropic"
    );
}

#[test]
fn infer_protocol_profile_supports_gemini_web_hint_and_base_url() {
    assert_eq!(
        infer_protocol_profile(
            "gemini_web_compatible",
            Some("gemini_web"),
            Some("https://gemini.google.com"),
        ),
        "gemini_web"
    );
    assert_eq!(
        infer_protocol_profile(
            "gemini_web_compatible",
            None,
            Some("https://gemini.google.com/app"),
        ),
        "gemini_web"
    );
    assert_eq!(
        infer_protocol_family(
            None,
            "gemini_web_compatible",
            Some("gemini_web"),
            Some("https://gemini.google.com/app"),
        ),
        GEMINI_WEB_CHAT_FAMILY
    );
}

#[test]
fn infer_protocol_profile_supports_chatgpt_web_reverse_hint_and_base_url() {
    assert_eq!(
        infer_protocol_profile(
            "chatgpt_web_reverse_compatible",
            Some("chatgpt_web_reverse"),
            Some("https://chatgpt.com"),
        ),
        "chatgpt_web_reverse"
    );
    assert_eq!(
        infer_protocol_profile(
            "chatgpt_web_reverse_compatible",
            None,
            Some("https://chatgpt.com/backend-api/conversation"),
        ),
        "chatgpt_web_reverse"
    );
    assert_eq!(
        infer_protocol_family(
            None,
            "chatgpt_web_reverse_compatible",
            Some("chatgpt_web_reverse"),
            Some("https://chatgpt.com/backend-api/conversation"),
        ),
        CHATGPT_WEB_CHAT_FAMILY
    );
}

#[test]
fn infer_protocol_profile_supports_chatgpt_official_and_codex_backend_lines() {
    assert_eq!(
        infer_protocol_profile(
            "openai_compatible",
            Some("openai"),
            Some("https://api.openai.com/v1"),
        ),
        "chatgpt_official_api"
    );
    assert_eq!(
        infer_protocol_profile("openai_compatible", None, Some("https://api.openai.com/v1"),),
        "chatgpt_official_api"
    );
    assert_eq!(
        infer_protocol_profile(
            "openai_compatible",
            Some("codex"),
            Some("https://chatgpt.com/backend-api/codex"),
        ),
        "chatgpt_codex_oauth_official_api"
    );
    assert_eq!(
        infer_protocol_profile(
            "openai_compatible",
            None,
            Some("https://chatgpt.com/backend-api/codex"),
        ),
        "chatgpt_codex_oauth_official_api"
    );
    assert_eq!(
        infer_protocol_family(
            None,
            "openai_compatible",
            Some("openai"),
            Some("https://api.openai.com/v1"),
        ),
        OPENAI_CHAT_FAMILY
    );
}

#[test]
fn infer_protocol_profile_prefers_strong_chatgpt_base_url_over_legacy_broad_hint() {
    assert_eq!(
        infer_protocol_profile(
            "openai_compatible",
            Some("openai"),
            Some("https://chatgpt.com/backend-api/codex"),
        ),
        "chatgpt_codex_oauth_official_api"
    );
    assert_eq!(
        infer_protocol_profile(
            "openai_compatible",
            Some("codex"),
            Some("https://api.openai.com/v1"),
        ),
        "chatgpt_official_api"
    );
    assert_eq!(
        infer_protocol_profile(
            "openai_compatible",
            Some("openai"),
            Some("https://chatgpt.com/backend-api/conversation"),
        ),
        "chatgpt_web_reverse"
    );
}

#[test]
fn infer_protocol_profile_supports_accio_hint_and_phoenix_url() {
    assert_eq!(
        infer_protocol_profile(
            "accio_compatible",
            Some("accio"),
            Some("https://api.openai.com/v1"),
        ),
        "accio"
    );
    assert_eq!(
        infer_protocol_profile(
            "accio_compatible",
            None,
            Some("https://phoenix-gw.alibaba.com"),
        ),
        "accio"
    );
    assert_eq!(
        infer_protocol_family(
            None,
            "accio_compatible",
            Some("accio"),
            Some("https://phoenix-gw.alibaba.com"),
        ),
        OPENAI_RESPONSES_FAMILY
    );
}

#[test]
fn infer_protocol_profile_supports_nvidia_hint_and_official_url() {
    assert_eq!(
        infer_protocol_profile(
            "openai_compatible",
            Some("nvidia"),
            Some("https://api.openai.com/v1"),
        ),
        "nvidia"
    );
    assert_eq!(
        infer_protocol_profile(
            "openai_compatible",
            Some("nvidia-nim"),
            Some("https://api.openai.com/v1"),
        ),
        "nvidia"
    );
    assert_eq!(
        infer_protocol_profile(
            "openai_compatible",
            None,
            Some("https://integrate.api.nvidia.com"),
        ),
        "nvidia"
    );
}

#[test]
fn protocol_family_selector_matches_generic_openai_and_search_groups() {
    assert!(protocol_family_selector_matches(
        "openai",
        "openai_responses"
    ));
    assert!(protocol_family_selector_matches(
        "openai",
        "openai_embeddings"
    ));
    assert!(protocol_family_selector_matches(
        "search_api",
        "linkup_search"
    ));
    assert!(protocol_family_selector_matches("search", "tavily_search"));
    assert!(!protocol_family_selector_matches(
        "perplexity_search",
        "exa_search"
    ));
}

#[test]
fn infer_protocol_family_preserves_special_provider_native_families() {
    assert_eq!(
        infer_protocol_family(None, "udio_compatible", Some("udio"), None),
        UDIO_MUSIC_FAMILY
    );
    assert_eq!(
        infer_protocol_family(
            None,
            "gemini_canvas_compatible",
            Some("gemini_canvas"),
            None
        ),
        GEMINI_CANVAS_IMAGES_FAMILY
    );
}

#[test]
fn lumalabs_selector_matches_all_native_media_families() {
    assert!(protocol_family_selector_matches(
        "lumalabs",
        LUMALABS_IMAGES_FAMILY,
    ));
    assert!(protocol_family_selector_matches(
        "lumalabs",
        LUMALABS_AUDIO_FAMILY,
    ));
    assert!(protocol_family_selector_matches(
        "lumalabs",
        LUMALABS_VIDEOS_FAMILY,
    ));
}

#[test]
fn udio_selector_matches_all_native_media_families() {
    assert!(protocol_family_selector_matches("udio", UDIO_IMAGES_FAMILY));
    assert!(protocol_family_selector_matches("udio", UDIO_MUSIC_FAMILY));
    assert!(protocol_family_selector_matches("udio", UDIO_VIDEOS_FAMILY));
}

#[test]
fn suno_selector_matches_all_native_media_families() {
    assert!(protocol_family_selector_matches("suno", SUNO_IMAGES_FAMILY));
    assert!(protocol_family_selector_matches("suno", SUNO_MUSIC_FAMILY));
    assert!(protocol_family_selector_matches("suno", SUNO_VIDEOS_FAMILY));
}

#[test]
fn producer_selector_matches_all_native_media_families() {
    assert!(protocol_family_selector_matches(
        "producer",
        PRODUCER_IMAGES_FAMILY
    ));
    assert!(protocol_family_selector_matches(
        "producer",
        PRODUCER_MUSIC_FAMILY
    ));
    assert!(protocol_family_selector_matches(
        "producer",
        PRODUCER_VIDEOS_FAMILY
    ));
}
