//! Classify provider accounts into folder families and surfaces.

use super::canonicalization::{canonicalize_folder_service_provider_slug, sanitize_file_component};
use crate::db;
use crate::protocol::gemini::shared::{
    GEMINI_API_MODULAR_PROFILE, GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE,
    GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE, GEMINI_WEB_REVERSE_MODULAR_PROFILE,
};
pub(super) trait ProviderAccountDescriptor {
    fn id(&self) -> &str;
    fn label(&self) -> &str;
    fn adapter(&self) -> &str;
    fn protocol_family(&self) -> &str;
    fn protocol_profile(&self) -> &str;
    fn service_provider_key(&self) -> &str;
    fn service_provider_label(&self) -> &str;
    fn source_kind(&self) -> Option<&str>;
    fn web_reverse_access_mode(&self) -> Option<&str>;
    fn payload_base_url(&self) -> Option<&str>;
}

impl ProviderAccountDescriptor for db::GatewayProviderAccountView {
    fn id(&self) -> &str {
        &self.id
    }

    fn label(&self) -> &str {
        &self.label
    }
    fn adapter(&self) -> &str {
        &self.adapter
    }
    fn protocol_family(&self) -> &str {
        &self.protocol_family
    }
    fn protocol_profile(&self) -> &str {
        &self.protocol_profile
    }
    fn service_provider_key(&self) -> &str {
        &self.service_provider_key
    }
    fn service_provider_label(&self) -> &str {
        &self.service_provider_label
    }
    fn source_kind(&self) -> Option<&str> {
        self.source_kind.as_deref()
    }
    fn web_reverse_access_mode(&self) -> Option<&str> {
        self.web_reverse_access_mode.as_deref()
    }
    fn payload_base_url(&self) -> Option<&str> {
        self.payload
            .get("baseUrl")
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                self.payload
                    .get("base_url")
                    .and_then(serde_json::Value::as_str)
            })
    }
}

impl ProviderAccountDescriptor for db::ProviderAccountFolderSyncMetadata {
    fn id(&self) -> &str {
        &self.id
    }

    fn label(&self) -> &str {
        &self.label
    }
    fn adapter(&self) -> &str {
        &self.adapter
    }
    fn protocol_family(&self) -> &str {
        &self.protocol_family
    }
    fn protocol_profile(&self) -> &str {
        &self.protocol_profile
    }
    fn service_provider_key(&self) -> &str {
        &self.service_provider_key
    }
    fn service_provider_label(&self) -> &str {
        &self.service_provider_label
    }
    fn source_kind(&self) -> Option<&str> {
        self.source_kind.as_deref()
    }
    fn web_reverse_access_mode(&self) -> Option<&str> {
        self.web_reverse_access_mode.as_deref()
    }
    fn payload_base_url(&self) -> Option<&str> {
        self.payload_base_url.as_deref()
    }
}

pub(super) fn derive_provider_family_slug<A: ProviderAccountDescriptor + ?Sized>(
    provider_account: &A,
) -> String {
    let label = provider_account.label().to_ascii_lowercase();
    let adapter = provider_account.adapter().to_ascii_lowercase();
    let protocol_profile = provider_account.protocol_profile().to_ascii_lowercase();
    let protocol_family = provider_account.protocol_family().to_ascii_lowercase();
    let service_provider_key = provider_account.service_provider_key().to_ascii_lowercase();
    let source_kind = provider_account
        .source_kind()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let web_reverse_access_mode = provider_account
        .web_reverse_access_mode()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let payload_base_url = provider_account
        .payload_base_url()
        .unwrap_or_default()
        .to_ascii_lowercase();

    if let Some(family_slug) = derive_gemini_provider_family_slug(
        &adapter,
        &protocol_profile,
        &service_provider_key,
        &source_kind,
        &web_reverse_access_mode,
        &payload_base_url,
    ) {
        return family_slug;
    }

    if adapter == "accio_compatible"
        || protocol_profile == "accio"
        || service_provider_key == "accio_platform"
        || payload_base_url.contains("phoenix-gw.alibaba.com")
        || label.contains("accio")
    {
        return "accio".to_string();
    }
    if adapter == "qwen_web_compatible"
        || protocol_profile == "qwen_web_chat"
        || protocol_family == "qwen_web_chat"
        || (service_provider_key == "qwen_platform"
            && source_kind == "web_reverse_api"
            && web_reverse_access_mode == "direct_http_replay")
        || payload_base_url.contains("chat.qwen.ai")
    {
        return "qwen-web-chat".to_string();
    }
    if adapter == "gemini_web_compatible"
        || protocol_profile == "gemini_web"
        || protocol_family == "gemini_web_chat"
        || (service_provider_key == "gemini_platform"
            && source_kind == "web_reverse_api"
            && web_reverse_access_mode == "direct_http_replay"
            && payload_base_url.contains("gemini.google.com"))
    {
        return "gemini-web-chat".to_string();
    }
    if adapter == "chatgpt_web_reverse_compatible"
        || protocol_profile == "chatgpt_web_reverse"
        || protocol_family == "chatgpt_web_chat"
        || (service_provider_key == "chatgpt_platform"
            && source_kind == "web_reverse_api"
            && web_reverse_access_mode == "direct_http_replay")
        || (payload_base_url.contains("chatgpt.com")
            && payload_base_url.contains("/backend-api/conversation"))
    {
        return "chatgpt-web-reverse".to_string();
    }
    if protocol_profile == "nvidia"
        || service_provider_key == "nvidia_platform"
        || payload_base_url.contains("integrate.api.nvidia.com")
        || payload_base_url.contains("api.nvidia.com")
        || label.contains("nvidia")
    {
        return "nvidia".to_string();
    }
    if protocol_profile == "grok_web"
        || service_provider_key == "grok_platform"
        || payload_base_url.contains("grok.com")
        || label.contains("grok")
    {
        return "grok".to_string();
    }
    if adapter == "suno_compatible"
        || protocol_profile == "suno"
        || service_provider_key == "suno_platform"
        || payload_base_url.contains("suno.com")
        || label.contains("suno")
    {
        return "suno".to_string();
    }
    if adapter == "udio_compatible"
        || protocol_profile == "udio"
        || service_provider_key == "udio_platform"
        || payload_base_url.contains("udio.com")
        || label.contains("udio")
    {
        return "udio".to_string();
    }
    if adapter == "lumalabs_compatible"
        || protocol_profile == "lumalabs"
        || service_provider_key == "lumalabs_platform"
        || payload_base_url.contains("lumalabs.ai")
        || label.contains("lumalabs")
        || label.contains("luma labs")
    {
        return "lumalabs".to_string();
    }
    if protocol_profile == "xai"
        || protocol_profile == "xai_openai"
        || service_provider_key == "xai_platform"
        || payload_base_url.contains("api.x.ai")
        || label.contains("xai")
        || label.contains("x.ai")
    {
        return "xai".to_string();
    }
    if protocol_profile == "perplexity_chat"
        || service_provider_key == "perplexity_platform"
        || payload_base_url.contains("api.perplexity.ai")
        || label.contains("perplexity")
    {
        return "perplexity".to_string();
    }
    if adapter == "freebuff_compatible"
        || protocol_profile == "freebuff"
        || service_provider_key == "freebuff_platform"
        || payload_base_url.contains("codebuff.com")
        || label.contains("freebuff")
        || label.contains("codebuff")
    {
        return "freebuff".to_string();
    }
    if protocol_profile == "xfyun_openai"
        || protocol_profile == "xfyun_native_websocket"
        || service_provider_key == "xfyun_platform"
        || payload_base_url.contains("xf-yun.com")
        || payload_base_url.contains("xfyun.cn")
        || label.contains("xfyun")
        || label.contains("讯飞")
    {
        return "xfyun".to_string();
    }
    if adapter == "producer_compatible"
        || protocol_profile == "producer"
        || service_provider_key == "producer_platform"
        || payload_base_url.contains("flowmusic.app")
        || payload_base_url.contains("producer.ai")
        || label.contains("producer")
    {
        return "producer".to_string();
    }
    if adapter == "kiro_compatible"
        || protocol_profile == "kiro"
        || service_provider_key == "kiro_platform"
        || payload_base_url.contains("codewhisperer")
        || label.contains("kiro")
    {
        return "kiro".to_string();
    }
    if label.contains("codex") || payload_base_url.contains("/backend-api/codex") {
        return "codex".to_string();
    }
    if payload_base_url.contains("anthropic") {
        return "anthropic".to_string();
    }
    if payload_base_url.contains("openai") {
        return "openai".to_string();
    }
    if payload_base_url.contains("cohere") {
        return "cohere".to_string();
    }
    if payload_base_url.contains("gemini") || payload_base_url.contains("googleapis.com") {
        return "gemini".to_string();
    }

    sanitize_file_component(provider_account.label())
}

pub(super) fn derive_service_provider_slug<A: ProviderAccountDescriptor + ?Sized>(
    provider_account: &A,
) -> String {
    if !provider_account.service_provider_key().trim().is_empty() {
        return canonicalize_folder_service_provider_slug(provider_account.service_provider_key());
    }
    canonicalize_folder_service_provider_slug(provider_account.service_provider_label())
}

fn derive_gemini_provider_family_slug(
    adapter: &str,
    protocol_profile: &str,
    service_provider_key: &str,
    source_kind: &str,
    web_reverse_access_mode: &str,
    payload_base_url: &str,
) -> Option<String> {
    if adapter == "gemini_web_compatible"
        || protocol_profile == "gemini_web"
        || protocol_profile == GEMINI_WEB_REVERSE_MODULAR_PROFILE
        || (service_provider_key == "gemini_platform"
            && source_kind == "web_reverse_api"
            && web_reverse_access_mode == "direct_http_replay"
            && payload_base_url.contains("gemini.google.com"))
    {
        return Some("gemini-web-chat".to_string());
    }

    if matches!(
        protocol_profile,
        "google_gemini_api"
            | GEMINI_API_MODULAR_PROFILE
            | "google_vertex_gemini"
            | "gemini_business"
            | "gemini_canvas"
            | GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE
            | GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE
    ) || service_provider_key == "gemini_platform"
        || payload_base_url.contains("gemini")
        || payload_base_url.contains("googleapis.com")
    {
        return Some("gemini".to_string());
    }

    None
}

fn derive_legacy_gemini_canvas_surface_slug(label: &str) -> String {
    if label.contains("chat") || label.contains("tts") {
        "gemini-canvas-chat-tts".to_string()
    } else if label.contains("image") {
        "gemini-canvas-images".to_string()
    } else if label.contains("music") {
        "gemini-canvas-music".to_string()
    } else if label.contains("video") {
        "gemini-canvas-videos".to_string()
    } else {
        "gemini-canvas".to_string()
    }
}

fn derive_gemini_provider_surface_slug(protocol_profile: &str, label: &str) -> Option<String> {
    match protocol_profile {
        "google_gemini_api" => Some("google-gemini-api".to_string()),
        GEMINI_API_MODULAR_PROFILE => Some("google-gemini-api-modular".to_string()),
        "google_vertex_gemini" => Some("google-vertex-gemini".to_string()),
        "gemini_web" => Some("gemini-web-chat".to_string()),
        GEMINI_WEB_REVERSE_MODULAR_PROFILE => Some("gemini-web-chat-modular".to_string()),
        "gemini_business" => Some("gemini-business-images".to_string()),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE => {
            Some("gemini-canvas-browser-relay".to_string())
        }
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE => {
            Some("gemini-canvas-program-relay".to_string())
        }
        "gemini_canvas" => Some(derive_legacy_gemini_canvas_surface_slug(label)),
        _ => None,
    }
}

pub(super) fn derive_provider_surface_slug<A: ProviderAccountDescriptor + ?Sized>(
    provider_account: &A,
) -> String {
    let protocol_profile = provider_account.protocol_profile().to_ascii_lowercase();
    let protocol_family = provider_account.protocol_family().to_ascii_lowercase();
    let service_provider_key = provider_account.service_provider_key().to_ascii_lowercase();
    let label = provider_account.label().to_ascii_lowercase();

    if service_provider_key == "gemini_platform" {
        if let Some(surface_slug) =
            derive_gemini_provider_surface_slug(protocol_profile.as_str(), label.as_str())
        {
            return surface_slug;
        }
    }

    match (service_provider_key.as_str(), protocol_profile.as_str()) {
        ("azure_openai_platform", "azure_openai") => "azure-openai".to_string(),
        ("anthropic_platform", "anthropic") => "anthropic-compatible".to_string(),
        ("aws_bedrock_platform", "aws_bedrock") => "bedrock-converse".to_string(),
        ("cohere_platform", "cohere") => "cohere-chat".to_string(),
        ("groq_platform", "groq") => "groq-openai".to_string(),
        ("nvidia_platform", "nvidia") => "nvidia-openai".to_string(),
        ("together_platform", "together") => "together-openai".to_string(),
        ("openrouter_platform", "openrouter") => "openrouter-openai".to_string(),
        ("muyuan_platform", "muyuan") => "muyuan-openai".to_string(),
        ("poe_platform", "poe") => "poe-openai".to_string(),
        ("longcat_platform", "longcat") => "longcat-openai".to_string(),
        ("deepseek_platform", "deepseek") => "deepseek-openai".to_string(),
        ("mistral_platform", "mistral") => "mistral-openai".to_string(),
        ("grok_platform", "grok_web") => "grok-web-reverse-api".to_string(),
        ("xai_platform", "xai") | ("xai_platform", "xai_openai") => "xai-openai".to_string(),
        ("perplexity_platform", "perplexity_search") => "perplexity-search".to_string(),
        ("perplexity_platform", "perplexity_chat") => "perplexity-chat".to_string(),
        ("tavily_platform", "tavily") => "tavily-search".to_string(),
        ("exa_platform", "exa") => "exa-search".to_string(),
        ("jina_platform", "jina_search") => "jina-search".to_string(),
        ("jina_platform", "jina_reader") => "jina-reader".to_string(),
        ("linkup_platform", "linkup") => "linkup-search".to_string(),
        ("you_platform", "you_search") => "you-search".to_string(),
        ("websearchapi_platform", "websearchapi") => "websearchapi-search".to_string(),
        ("qwen_platform", "qwen_dashscope_openai") => "qwen-dashscope-openai".to_string(),
        ("qwen_platform", "qwen_coding_plan_openai") => "qwen-coding-plan-openai".to_string(),
        ("qwen_platform", "qwen_coding_plan_anthropic") => "qwen-coding-plan-anthropic".to_string(),
        ("qwen_platform", "qwen_web_chat") => "qwen-web-chat".to_string(),
        ("chatgpt_platform", "chatgpt_official_api") => "chatgpt-official-api".to_string(),
        ("chatgpt_platform", "chatgpt_codex_backend") => "chatgpt-codex-backend".to_string(),
        ("chatgpt_platform", "chatgpt_web_reverse") => "chatgpt-web-reverse".to_string(),
        ("chataibot_platform", "chataibot") => "chataibot-images".to_string(),
        ("aistudio_platform", "aistudio_web_reverse") => "aistudio-web-reverse".to_string(),
        ("suno_platform", "suno") => "suno".to_string(),
        ("udio_platform", "udio") => "udio".to_string(),
        ("lumalabs_platform", "lumalabs") => "lumalabs".to_string(),
        ("freebuff_platform", "freebuff") => "freebuff-compatible".to_string(),
        ("xfyun_platform", "xfyun_openai") => "xfyun-openai".to_string(),
        ("xfyun_platform", "xfyun_native_websocket") => "xfyun-native-websocket".to_string(),
        ("producer_platform", "producer") => {
            if protocol_family.contains("image") || label.contains("image") {
                "producer-images".to_string()
            } else if protocol_family.contains("video") || label.contains("video") {
                "producer-videos".to_string()
            } else if protocol_family.contains("music") || label.contains("music") {
                "producer-music".to_string()
            } else {
                "producer".to_string()
            }
        }
        ("kiro_platform", "kiro") => "kiro-compatible".to_string(),
        _ if !provider_account.protocol_profile().trim().is_empty() => {
            sanitize_file_component(provider_account.protocol_profile())
        }
        _ => derive_provider_family_slug(provider_account),
    }
}
