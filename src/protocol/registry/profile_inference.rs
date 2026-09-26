//! Ordered provider-hint and base-URL profile inference.

use super::profiles::{canonicalize_protocol_profile_key, default_protocol_profile_for_adapter};

pub fn infer_protocol_profile(
    adapter: &str,
    provider_hint: Option<&str>,
    base_url: Option<&str>,
) -> String {
    let hinted = provider_hint
        .map(canonicalize_protocol_profile_key)
        .filter(|value| !value.is_empty());
    let hinted_profile = hinted.as_deref().and_then(profile_from_hint);
    let base_url_profile = base_url
        .map(|value| value.trim().to_lowercase())
        .as_deref()
        .and_then(|value| profile_from_base_url(adapter, value));

    match (hinted_profile, base_url_profile) {
        (Some(hinted), Some(base_url_profile))
            if should_prefer_base_url_profile(adapter, hinted, base_url_profile) =>
        {
            return base_url_profile.to_string();
        }
        (Some(hinted), _) => return hinted.to_string(),
        (None, Some(base_url_profile)) => return base_url_profile.to_string(),
        (None, None) => {}
    }
    canonicalize_protocol_profile_key(default_protocol_profile_for_adapter(adapter))
}

fn should_prefer_base_url_profile(
    adapter: &str,
    hinted_profile: &str,
    base_url_profile: &str,
) -> bool {
    hinted_profile != base_url_profile
        && ((is_chatgpt_profile(hinted_profile) && is_chatgpt_profile(base_url_profile))
            || (adapter.trim() == "chatgpt_web_reverse_compatible"
                && base_url_profile == "chatgpt_web_reverse"))
}

fn is_chatgpt_profile(profile: &str) -> bool {
    matches!(
        profile,
        "chatgpt_official_api" | "chatgpt_codex_oauth_official_api" | "chatgpt_web_reverse"
    )
}

fn profile_from_hint(value: &str) -> Option<&'static str> {
    match value {
        "openai" | "openai_platform" | "chatgpt_official_api" => Some("chatgpt_official_api"),
        "codex" | "chatgpt_codex_backend" | "chatgpt_codex_oauth_official_api" => {
            Some("chatgpt_codex_oauth_official_api")
        }
        "azure" | "azure_openai" => Some("azure_openai"),
        "anthropic" => Some("anthropic"),
        "google" | "gemini" | "google_gemini" | "google_gemini_api" | "aistudio_official_api" => {
            Some("aistudio_official_api")
        }
        "vertex"
        | "google_vertex"
        | "google_vertex_gemini"
        | "vertex_official_api"
        | "google_agent_platform"
        | "google_agent_platform_official_api" => Some("google_agent_platform_official_api"),
        "bedrock" | "aws_bedrock" => Some("aws_bedrock"),
        "cohere" => Some("cohere"),
        "groq" => Some("groq"),
        "together" => Some("together"),
        "openrouter" => Some("openrouter"),
        "deepseek" => Some("deepseek"),
        "mistral" => Some("mistral"),
        "xai" | "x_ai" => Some("xai"),
        "nvidia" | "nvidia_platform" | "nvidia_nim" => Some("nvidia"),
        "perplexity" | "perplexity_chat" => Some("perplexity_chat"),
        "perplexity_search" => Some("perplexity_search"),
        "tavily" => Some("tavily"),
        "exa" => Some("exa"),
        "jina" | "jina_search" => Some("jina_search"),
        "jina_reader" => Some("jina_reader"),
        "linkup" => Some("linkup"),
        "you" | "you_search" => Some("you_search"),
        "websearchapi" => Some("websearchapi"),
        "accio" | "accio_platform" => Some("accio"),
        "qwen" | "qwen_platform" => Some("qwen_dashscope_openai"),
        "qwen_dashscope" | "qwen_dashscope_openai" => Some("qwen_dashscope_openai"),
        "qwen_coding_plan" | "qwen_coding_plan_openai" => Some("qwen_coding_plan_openai"),
        "qwen_coding_plan_anthropic" => Some("qwen_coding_plan_anthropic"),
        "qwen_web" | "qwen_web_chat" => Some("qwen_web_chat"),
        "aistudio" | "aistudio_web" | "aistudio_web_reverse" => Some("aistudio_web_reverse"),
        "chatgpt_web" | "chatgpt_web_reverse" | "chatgpt_web_chat" => Some("chatgpt_web_reverse"),
        "gemini_web" | "gemini_web_chat" => Some("gemini_web"),
        "gemini_web_reverse_modular" => Some("gemini_web_reverse_modular"),
        "gemini_canvas_web_reverse_modular" => Some("gemini_canvas_web_reverse_modular"),
        "gemini_canvas_program_web_reverse_modular" => {
            Some("gemini_canvas_program_web_reverse_modular")
        }
        "kiro" => Some("kiro"),
        "freebuff" => Some("freebuff"),
        "producer" => Some("producer"),
        "gemini_canvas" => Some("gemini_canvas"),
        "suno" => Some("suno"),
        "udio" => Some("udio"),
        "xfyun" | "xfyun_openai" => Some("xfyun_openai"),
        "xfyun_native_websocket" => Some("xfyun_native_websocket"),
        _ => None,
    }
}

fn profile_from_base_url(adapter: &str, base_url: &str) -> Option<&'static str> {
    if base_url.contains("openai.azure.com")
        || (base_url.contains("azure.com") && base_url.contains("/openai/"))
        || base_url.contains(".cognitiveservices.azure.com")
    {
        return Some("azure_openai");
    }
    if base_url.contains("chatgpt.com/backend-api/codex") {
        return Some("chatgpt_codex_oauth_official_api");
    }
    if crate::protocol::chatgpt::official_api::is_chatgpt_official_api_base_url(base_url) {
        return Some("chatgpt_official_api");
    }
    if base_url.contains("chatgpt.com/backend-api/conversation")
        || base_url.contains("chatgpt.com/backend-api/models")
        || (adapter.trim() == "chatgpt_web_reverse_compatible" && base_url.contains("chatgpt.com"))
    {
        return Some("chatgpt_web_reverse");
    }
    if base_url.contains("phoenix-gw.alibaba.com") || base_url.contains("accio.com") {
        return Some("accio");
    }
    if base_url.contains("chat.qwen.ai") {
        return Some("qwen_web_chat");
    }
    if base_url.contains("ai.studio") || base_url.contains("aistudio.google.com") {
        return Some("aistudio_web_reverse");
    }
    if base_url.contains("coding.dashscope.aliyuncs.com/apps/anthropic") {
        return Some("qwen_coding_plan_anthropic");
    }
    if base_url.contains("coding.dashscope.aliyuncs.com") {
        return Some("qwen_coding_plan_openai");
    }
    if base_url.contains("dashscope.aliyuncs.com/compatible-mode/")
        || base_url.contains("dashscope-us.aliyuncs.com/compatible-mode/")
        || base_url.contains("dashscope-intl.aliyuncs.com/compatible-mode/")
    {
        return Some("qwen_dashscope_openai");
    }
    if base_url.contains("api.anthropic.com") {
        return Some("anthropic");
    }
    if base_url.contains("generativelanguage.googleapis.com") {
        return Some("aistudio_official_api");
    }
    if base_url.contains("aiplatform.googleapis.com")
        || base_url.contains("vertexai.googleapis.com")
        || base_url.contains("/publishers/google/models/")
    {
        return Some("google_agent_platform_official_api");
    }
    if base_url.contains("bedrock") && base_url.contains("amazonaws.com") {
        return Some("aws_bedrock");
    }
    if base_url.contains("api.cohere.ai") {
        return Some("cohere");
    }
    if base_url.contains("api.groq.com") || base_url.contains("console.groq.com") {
        return Some("groq");
    }
    if base_url.contains("api.together.xyz") || base_url.contains("api.together.ai") {
        return Some("together");
    }
    if base_url.contains("openrouter.ai") {
        return Some("openrouter");
    }
    if base_url.contains("muyuan.do") {
        return Some("muyuan");
    }
    if base_url.contains("api.poe.com") {
        return Some("poe");
    }
    if base_url.contains("api.longcat.chat") {
        return Some("longcat");
    }
    if base_url.contains("api.deepseek.com") {
        return Some("deepseek");
    }
    if base_url.contains("api.mistral.ai") {
        return Some("mistral");
    }
    if base_url.contains("api.x.ai") || base_url.contains("x.ai") {
        return Some("xai");
    }
    if base_url.contains("integrate.api.nvidia.com") || base_url.contains("api.nvidia.com") {
        return Some("nvidia");
    }
    if base_url.contains("api.perplexity.ai") {
        return Some(if adapter.trim() == "search_api_compatible" {
            "perplexity_search"
        } else {
            "perplexity_chat"
        });
    }
    if base_url.contains("api.tavily.com") {
        return Some("tavily");
    }
    if base_url.contains("api.exa.ai") {
        return Some("exa");
    }
    if base_url.contains("api.jina.ai") {
        return Some(if base_url.contains("reader") {
            "jina_reader"
        } else {
            "jina_search"
        });
    }
    if base_url.contains("api.linkup.so") {
        return Some("linkup");
    }
    if base_url.contains("api.ydc-index.io") || base_url.contains("you.com") {
        return Some("you_search");
    }
    if base_url.contains("websearchapi") {
        return Some("websearchapi");
    }
    if base_url.contains("business.gemini.google")
        || base_url.contains("biz-discoveryengine.googleapis.com")
    {
        return Some("gemini_business");
    }
    if base_url.contains("chataibot.pro") {
        return Some("chataibot");
    }
    if base_url.contains("lumalabs.ai") {
        return Some("lumalabs");
    }
    if adapter.trim() == "gemini_web_compatible" && base_url.contains("gemini.google.com") {
        return Some("gemini_web");
    }
    if base_url.contains("gemini.google.com") {
        return Some("gemini_canvas");
    }
    if base_url.contains("producer.ai") {
        return Some("producer");
    }
    if base_url.contains("suno.com") {
        return Some("suno");
    }
    if base_url.contains("udio.com") {
        return Some("udio");
    }
    None
}
