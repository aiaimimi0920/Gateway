//! Text-provider catalog aliases and default material layouts.
use super::super::canonicalization::canonicalize_folder_surface_slug;
use super::super::classification::{
    derive_provider_family_slug, derive_provider_surface_slug, derive_service_provider_slug,
};
use super::super::layout::default_folder_sync_relative_path;
use super::{build_test_credential, build_test_provider_account};

#[test]
fn derive_provider_surface_slug_maps_wave3_official_api_lines() {
    let mut azure = build_test_provider_account(
        "Azure OpenAI",
        "openai_compatible",
        "openai",
        "azure_openai",
        "https://example.openai.azure.com/openai/v1",
        Some("official_vendor_api"),
        None,
    );
    azure.service_provider_key = "azure_openai_platform".to_string();
    azure.service_provider_label = "Azure OpenAI".to_string();
    assert_eq!(derive_provider_surface_slug(&azure), "azure-openai");
    assert_eq!(
        canonicalize_folder_surface_slug("azure-openai-v1"),
        "azure-openai"
    );

    let mut anthropic = build_test_provider_account(
        "Anthropic Messages",
        "anthropic_compatible",
        "anthropic",
        "anthropic",
        "https://api.anthropic.com/v1",
        Some("official_model_api"),
        None,
    );
    anthropic.service_provider_key = "anthropic_platform".to_string();
    anthropic.service_provider_label = "Anthropic Messages".to_string();
    assert_eq!(
        derive_provider_surface_slug(&anthropic),
        "anthropic-compatible"
    );
    assert_eq!(
        canonicalize_folder_surface_slug("anthropic-messages"),
        "anthropic-compatible"
    );

    let mut bedrock = build_test_provider_account(
        "AWS Bedrock Converse",
        "bedrock_converse_compatible",
        "bedrock_converse",
        "aws_bedrock",
        "https://bedrock-runtime.us-east-1.amazonaws.com",
        Some("official_model_api"),
        None,
    );
    bedrock.service_provider_key = "aws_bedrock_platform".to_string();
    bedrock.service_provider_label = "AWS Bedrock Converse".to_string();
    assert_eq!(derive_provider_surface_slug(&bedrock), "bedrock-converse");

    let mut cohere = build_test_provider_account(
        "Cohere Chat",
        "cohere_compatible",
        "cohere_chat",
        "cohere",
        "https://api.cohere.com",
        Some("official_model_api"),
        None,
    );
    cohere.service_provider_key = "cohere_platform".to_string();
    cohere.service_provider_label = "Cohere Chat".to_string();
    assert_eq!(derive_provider_surface_slug(&cohere), "cohere-chat");
    assert_eq!(
        canonicalize_folder_surface_slug("cohere-chat-v2"),
        "cohere-chat"
    );
}

#[test]
fn derive_provider_surface_slug_maps_wave4_openai_compatible_lines() {
    let mut groq = build_test_provider_account(
        "Groq OpenAI-compatible",
        "openai_compatible",
        "openai",
        "groq",
        "https://api.groq.com/openai/v1",
        Some("official_vendor_api"),
        None,
    );
    groq.service_provider_key = "groq_platform".to_string();
    groq.service_provider_label = "Groq OpenAI-compatible".to_string();
    assert_eq!(derive_provider_surface_slug(&groq), "groq-openai");
    assert_eq!(canonicalize_folder_surface_slug("groq"), "groq-openai");

    let mut together = build_test_provider_account(
        "Together OpenAI-compatible",
        "openai_compatible",
        "openai",
        "together",
        "https://api.together.xyz/v1",
        Some("aggregator_api"),
        None,
    );
    together.service_provider_key = "together_platform".to_string();
    together.service_provider_label = "Together OpenAI-compatible".to_string();
    assert_eq!(derive_provider_surface_slug(&together), "together-openai");
    assert_eq!(
        canonicalize_folder_surface_slug("together"),
        "together-openai"
    );

    let mut openrouter = build_test_provider_account(
        "OpenRouter OpenAI-compatible",
        "openai_compatible",
        "openai",
        "openrouter",
        "https://openrouter.ai/api/v1",
        Some("aggregator_api"),
        None,
    );
    openrouter.service_provider_key = "openrouter_platform".to_string();
    openrouter.service_provider_label = "OpenRouter OpenAI-compatible".to_string();
    assert_eq!(
        derive_provider_surface_slug(&openrouter),
        "openrouter-openai"
    );
    assert_eq!(
        canonicalize_folder_surface_slug("openrouter"),
        "openrouter-openai"
    );

    let mut deepseek = build_test_provider_account(
        "DeepSeek OpenAI-compatible",
        "openai_compatible",
        "openai",
        "deepseek",
        "https://api.deepseek.com/v1",
        Some("official_model_api"),
        None,
    );
    deepseek.service_provider_key = "deepseek_platform".to_string();
    deepseek.service_provider_label = "DeepSeek OpenAI-compatible".to_string();
    assert_eq!(derive_provider_surface_slug(&deepseek), "deepseek-openai");
    assert_eq!(
        canonicalize_folder_surface_slug("deepseek"),
        "deepseek-openai"
    );

    let mut mistral = build_test_provider_account(
        "Mistral OpenAI-compatible",
        "openai_compatible",
        "openai",
        "mistral",
        "https://api.mistral.ai/v1",
        Some("official_model_api"),
        None,
    );
    mistral.service_provider_key = "mistral_platform".to_string();
    mistral.service_provider_label = "Mistral OpenAI-compatible".to_string();
    assert_eq!(derive_provider_surface_slug(&mistral), "mistral-openai");
    assert_eq!(
        canonicalize_folder_surface_slug("mistral"),
        "mistral-openai"
    );
}

#[test]
fn derive_provider_surface_slug_maps_nvidia_and_grok_lines() {
    let mut nvidia = build_test_provider_account(
        "NVIDIA OpenAI-compatible",
        "openai_compatible",
        "openai",
        "nvidia",
        "https://integrate.api.nvidia.com/v1",
        Some("official_vendor_api"),
        None,
    );
    nvidia.service_provider_key = "nvidia_platform".to_string();
    nvidia.service_provider_label = "NVIDIA OpenAI-compatible".to_string();
    assert_eq!(derive_provider_family_slug(&nvidia), "nvidia");
    assert_eq!(derive_provider_surface_slug(&nvidia), "nvidia-openai");
    assert_eq!(
        canonicalize_folder_surface_slug("nvidia-nim"),
        "nvidia-openai"
    );

    let mut grok = build_test_provider_account(
        "Grok Web",
        "grok_compatible",
        "openai",
        "grok_web",
        "https://grok.com",
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    );
    grok.service_provider_key = "grok_platform".to_string();
    grok.service_provider_label = "Grok Web".to_string();
    assert_eq!(derive_provider_family_slug(&grok), "grok");
    assert_eq!(derive_provider_surface_slug(&grok), "grok-web-reverse-api");
    assert_eq!(
        canonicalize_folder_surface_slug("grok-web"),
        "grok-web-reverse-api"
    );
}

#[test]
fn derive_provider_surface_slug_maps_remaining_unfinished_platform_lines() {
    let mut xai = build_test_provider_account(
        "xAI OpenAI",
        "openai_compatible",
        "openai_compatible",
        "xai",
        "https://api.x.ai/v1",
        Some("official_vendor_api"),
        None,
    );
    xai.service_provider_key = "xai_platform".to_string();
    xai.service_provider_label = "xAI Platform".to_string();
    assert_eq!(derive_provider_family_slug(&xai), "xai");
    assert_eq!(derive_service_provider_slug(&xai), "xai-platform");
    assert_eq!(derive_provider_surface_slug(&xai), "xai-openai");
    assert_eq!(canonicalize_folder_surface_slug("xai"), "xai-openai");
    assert_eq!(
        default_folder_sync_relative_path(
            &xai,
            &build_test_credential("cred-xai", "folder_sync", None, "active", None),
        ),
        "xai-platform/xai-openai/api-key/cred-xai.json"
    );

    let mut perplexity_chat = build_test_provider_account(
        "Perplexity Chat",
        "openai_compatible",
        "openai_compatible",
        "perplexity_chat",
        "https://api.perplexity.ai",
        Some("official_vendor_api"),
        None,
    );
    perplexity_chat.service_provider_key = "perplexity_platform".to_string();
    perplexity_chat.service_provider_label = "Perplexity Platform".to_string();
    assert_eq!(derive_provider_family_slug(&perplexity_chat), "perplexity");
    assert_eq!(
        derive_service_provider_slug(&perplexity_chat),
        "perplexity-platform"
    );
    assert_eq!(
        derive_provider_surface_slug(&perplexity_chat),
        "perplexity-chat"
    );
    assert_eq!(
        default_folder_sync_relative_path(
            &perplexity_chat,
            &build_test_credential("cred-perplexity", "folder_sync", None, "active", None),
        ),
        "perplexity-platform/perplexity-chat/api-key/cred-perplexity.json"
    );

    let mut freebuff = build_test_provider_account(
        "FreeBuff",
        "freebuff_compatible",
        "freebuff",
        "freebuff",
        "https://www.codebuff.com",
        Some("web_reverse_api"),
        Some("browser_challenge"),
    );
    freebuff.service_provider_key = "freebuff_platform".to_string();
    freebuff.service_provider_label = "FreeBuff".to_string();
    assert_eq!(derive_provider_family_slug(&freebuff), "freebuff");
    assert_eq!(
        derive_provider_surface_slug(&freebuff),
        "freebuff-compatible"
    );
    assert_eq!(
        default_folder_sync_relative_path(
            &freebuff,
            &build_test_credential("cred-freebuff", "folder_sync", None, "active", None),
        ),
        "freebuff-platform/freebuff-compatible/session-auth/cred-freebuff.json"
    );

    let mut xfyun_openai = build_test_provider_account(
        "XFYun OpenAI",
        "openai_compatible",
        "openai_compatible",
        "xfyun_openai",
        "https://spark-api-open.xf-yun.com/v1",
        Some("official_vendor_api"),
        None,
    );
    xfyun_openai.service_provider_key = "xfyun_platform".to_string();
    xfyun_openai.service_provider_label = "XFYun Platform".to_string();
    assert_eq!(derive_provider_family_slug(&xfyun_openai), "xfyun");
    assert_eq!(derive_provider_surface_slug(&xfyun_openai), "xfyun-openai");
    assert_eq!(
        default_folder_sync_relative_path(
            &xfyun_openai,
            &build_test_credential("cred-xfyun-openai", "folder_sync", None, "active", None),
        ),
        "xfyun-platform/xfyun-openai/api-key/cred-xfyun-openai.json"
    );

    let mut xfyun_native = build_test_provider_account(
        "XFYun Native WebSocket",
        "xfyun_websocket_compatible",
        "xfyun_websocket",
        "xfyun_native_websocket",
        "wss://spark-api.xf-yun.com/v1.1/chat",
        Some("official_vendor_api"),
        None,
    );
    xfyun_native.service_provider_key = "xfyun_platform".to_string();
    xfyun_native.service_provider_label = "XFYun Platform".to_string();
    assert_eq!(derive_provider_family_slug(&xfyun_native), "xfyun");
    assert_eq!(
        derive_provider_surface_slug(&xfyun_native),
        "xfyun-native-websocket"
    );
    assert_eq!(
        default_folder_sync_relative_path(
            &xfyun_native,
            &build_test_credential("cred-xfyun-native", "folder_sync", None, "active", None),
        ),
        "xfyun-platform/xfyun-native-websocket/api-key/cred-xfyun-native.json"
    );

    let mut producer_music = build_test_provider_account(
        "Producer Music",
        "producer_compatible",
        "producer_music",
        "producer",
        "https://www.flowmusic.app",
        Some("web_reverse_api"),
        Some("browser_challenge"),
    );
    producer_music.service_provider_key = "producer_platform".to_string();
    producer_music.service_provider_label = "Producer.ai Platform".to_string();
    assert_eq!(derive_provider_family_slug(&producer_music), "producer");
    assert_eq!(
        derive_provider_surface_slug(&producer_music),
        "producer-music"
    );
    assert_eq!(
        canonicalize_folder_surface_slug("producer-videos"),
        "producer-videos"
    );
    assert_eq!(
        default_folder_sync_relative_path(
            &producer_music,
            &build_test_credential("cred-producer", "folder_sync", None, "active", None),
        ),
        "producer-platform/producer-music/session-auth/cred-producer.json"
    );

    let mut kiro = build_test_provider_account(
        "Kiro-compatible",
        "kiro_compatible",
        "kiro",
        "kiro",
        "https://codewhisperer.us-east-1.amazonaws.com",
        Some("official_vendor_api"),
        None,
    );
    kiro.service_provider_key = "kiro_platform".to_string();
    kiro.service_provider_label = "Kiro Platform".to_string();
    assert_eq!(derive_provider_family_slug(&kiro), "kiro");
    assert_eq!(derive_provider_surface_slug(&kiro), "kiro-compatible");
    assert_eq!(
        default_folder_sync_relative_path(
            &kiro,
            &build_test_credential("cred-kiro", "folder_sync", None, "active", None),
        ),
        "kiro-platform/kiro-compatible/bearer-token/cred-kiro.json"
    );
}
