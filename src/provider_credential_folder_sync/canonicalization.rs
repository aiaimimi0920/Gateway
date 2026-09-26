//! Canonical names shared by folder imports, exports and provider matching.
//! Alias namespaces stay separate for families, services, surfaces and materials.

pub(super) fn canonicalize_folder_family_slug(value: &str) -> String {
    let slug = sanitize_file_component(value);
    match slug.as_str() {
        "accio-platform" | "accio-manager" | "phoenix-gw" => "accio".to_string(),
        "qwen"
        | "qwen-web"
        | "qwen-webui"
        | "qwen-web-chat"
        | "qwen-webui-replay"
        | "qwen-webui-replay-live" => "qwen-web-chat".to_string(),
        "aistudio" | "ai-studio" | "aistudio-web-reverse" => "aistudio-web-reverse".to_string(),
        "gemini-web" | "gemini-web-chat" => "gemini-web-chat".to_string(),
        "chatgpt" | "chatgpt-web" | "chatgpt-web-reverse" => "chatgpt-web-reverse".to_string(),
        "nvidia-platform" | "nvidia-nim" => "nvidia".to_string(),
        "grok-platform" | "grok-web" | "grok-web-reverse" => "grok".to_string(),
        "suno-platform" | "suno-music" | "suno-images" | "suno-videos" => "suno".to_string(),
        "udio-platform" | "udio-music" | "udio-images" | "udio-videos" => "udio".to_string(),
        "xai-platform" | "xai-openai" => "xai".to_string(),
        "perplexity-platform" | "perplexity-chat" | "perplexity-search" => "perplexity".to_string(),
        "freebuff-platform" | "freebuff-compatible" | "codebuff" => "freebuff".to_string(),
        "xfyun-platform" | "xfyun-openai" | "xfyun-native-websocket" | "xfyun-websocket" => {
            "xfyun".to_string()
        }
        "producer-platform" | "producer-images" | "producer-music" | "producer-videos" => {
            "producer".to_string()
        }
        "kiro-platform" | "kiro-compatible" => "kiro".to_string(),
        "lumalabs-platform" | "luma" | "luma-labs" | "luma-labs-images" | "luma-labs-videos"
        | "luma-labs-audio" | "lumalabs-images" | "lumalabs-videos" | "lumalabs-audio" => {
            "lumalabs".to_string()
        }
        _ => slug,
    }
}

pub(super) fn canonicalize_folder_service_provider_slug(value: &str) -> String {
    let slug = sanitize_file_component(value);
    match slug.as_str() {
        "azure-openai" | "azure-openai-platform" | "azure-openai-service" => {
            "azure-openai-platform".to_string()
        }
        "anthropic" | "anthropic-platform" => "anthropic-platform".to_string(),
        "aws-bedrock" | "aws-bedrock-platform" | "bedrock-platform" => {
            "aws-bedrock-platform".to_string()
        }
        "cohere" | "cohere-platform" => "cohere-platform".to_string(),
        "groq" | "groq-platform" => "groq-platform".to_string(),
        "grok" | "grok-platform" | "grok-web" => "grok-platform".to_string(),
        "together" | "together-platform" => "together-platform".to_string(),
        "openrouter" | "openrouter-platform" => "openrouter-platform".to_string(),
        "deepseek" | "deepseek-platform" => "deepseek-platform".to_string(),
        "mistral" | "mistral-platform" => "mistral-platform".to_string(),
        "qwen" | "qwen-platform" => "qwen-platform".to_string(),
        "chatgpt" | "chatgpt-platform" => "chatgpt-platform".to_string(),
        "aistudio" | "ai-studio" | "aistudio-platform" => "aistudio-platform".to_string(),
        "gemini" | "gemini-platform" | "google-gemini" => "gemini-platform".to_string(),
        "suno" | "suno-platform" => "suno-platform".to_string(),
        "udio" | "udio-platform" => "udio-platform".to_string(),
        "xai" | "xai-platform" | "xai-openai" => "xai-platform".to_string(),
        "perplexity" | "perplexity-platform" | "perplexity-chat" | "perplexity-search" => {
            "perplexity-platform".to_string()
        }
        "freebuff" | "freebuff-platform" | "freebuff-compatible" | "codebuff" => {
            "freebuff-platform".to_string()
        }
        "xfyun" | "xfyun-platform" | "xfyun-openai" | "xfyun-native-websocket" => {
            "xfyun-platform".to_string()
        }
        "producer" | "producer-platform" | "producer-images" | "producer-music"
        | "producer-videos" => "producer-platform".to_string(),
        "kiro" | "kiro-platform" | "kiro-compatible" => "kiro-platform".to_string(),
        "luma" | "luma-labs" | "lumalabs" | "lumalabs-platform" => "lumalabs-platform".to_string(),
        _ => slug,
    }
}

pub(super) fn canonicalize_folder_surface_slug(value: &str) -> String {
    let slug = sanitize_file_component(value);
    match slug.as_str() {
        "azure-openai" | "azure-openai-v1" => "azure-openai".to_string(),
        "anthropic-compatible" | "anthropic-messages" => "anthropic-compatible".to_string(),
        "bedrock-converse" | "aws-bedrock-converse" => "bedrock-converse".to_string(),
        "cohere-chat" | "cohere-chat-v2" | "cohere" => "cohere-chat".to_string(),
        "groq" | "groq-openai" => "groq-openai".to_string(),
        "nvidia" | "nvidia-openai" | "nvidia-nim" => "nvidia-openai".to_string(),
        "together" | "together-openai" => "together-openai".to_string(),
        "openrouter" | "openrouter-openai" => "openrouter-openai".to_string(),
        "deepseek" | "deepseek-openai" => "deepseek-openai".to_string(),
        "mistral" | "mistral-openai" => "mistral-openai".to_string(),
        "grok" | "grok-web" | "grok-web-reverse-api" => "grok-web-reverse-api".to_string(),
        "xai" | "xai-openai" => "xai-openai".to_string(),
        "perplexity-chat" | "perplexity-openai" => "perplexity-chat".to_string(),
        "perplexity-search" => "perplexity-search".to_string(),
        "freebuff" | "freebuff-compatible" | "codebuff" => "freebuff-compatible".to_string(),
        "xfyun" | "xfyun-openai" => "xfyun-openai".to_string(),
        "xfyun-native" | "xfyun-websocket" | "xfyun-native-websocket" => {
            "xfyun-native-websocket".to_string()
        }
        "producer-images" | "producer-image" => "producer-images".to_string(),
        "producer-music" => "producer-music".to_string(),
        "producer-videos" | "producer-video" => "producer-videos".to_string(),
        "producer-web-reverse-api" => "producer".to_string(),
        "kiro" | "kiro-compatible" => "kiro-compatible".to_string(),
        "qwen-dashscope" | "qwen-dashscope-openai" => "qwen-dashscope-openai".to_string(),
        "qwen-coding-plan-openai" | "qwen-coding-openai" => "qwen-coding-plan-openai".to_string(),
        "qwen-coding-plan-anthropic" | "qwen-coding-anthropic" => {
            "qwen-coding-plan-anthropic".to_string()
        }
        "chatgpt-official-api" | "openai-platform" => "chatgpt-official-api".to_string(),
        "chatgpt-codex-backend" => "chatgpt-codex-backend".to_string(),
        "chataibot" | "chataibot-images" => "chataibot-images".to_string(),
        "aistudio" | "ai-studio" | "aistudio-web-reverse" => "aistudio-web-reverse".to_string(),
        "google-gemini-api" | "gemini-api" => "google-gemini-api".to_string(),
        "google-gemini-api-modular" | "gemini-api-modular" => {
            "google-gemini-api-modular".to_string()
        }
        "google-vertex-gemini" | "vertex-gemini" => "google-vertex-gemini".to_string(),
        "gemini-business" | "gemini-business-images" => "gemini-business-images".to_string(),
        "gemini-web" | "gemini-web-chat" => "gemini-web-chat".to_string(),
        "gemini-web-chat-modular" | "gemini-web-modular" => "gemini-web-chat-modular".to_string(),
        "gemini-canvas-chat" | "gemini-canvas-chat-tts" => "gemini-canvas-chat-tts".to_string(),
        "gemini-canvas-browser-relay" | "gemini-canvas-web-reverse" => {
            "gemini-canvas-browser-relay".to_string()
        }
        "gemini-canvas-program-relay" | "gemini-canvas-program-web-reverse" => {
            "gemini-canvas-program-relay".to_string()
        }
        "gemini-canvas-images" | "gemini-canvas-image" => "gemini-canvas-images".to_string(),
        "gemini-canvas-music" => "gemini-canvas-music".to_string(),
        "gemini-canvas-video" | "gemini-canvas-videos" => "gemini-canvas-videos".to_string(),
        "suno" | "suno-music" | "suno-images" | "suno-videos" | "suno-web-reverse-api" => {
            "suno".to_string()
        }
        "udio" | "udio-music" | "udio-images" | "udio-videos" | "udio-web-reverse-api" => {
            "udio".to_string()
        }
        "luma"
        | "luma-labs"
        | "luma-labs-images"
        | "luma-labs-videos"
        | "luma-labs-audio"
        | "lumalabs"
        | "lumalabs-images"
        | "lumalabs-videos"
        | "lumalabs-audio"
        | "lumalabs-web-reverse-api" => "lumalabs".to_string(),
        _ => canonicalize_folder_family_slug(slug.as_str()),
    }
}

pub(super) fn canonicalize_credential_material_kind(value: &str) -> String {
    let slug = sanitize_file_component(value);
    match slug.as_str() {
        "api-key" | "apikey" => "api_key".to_string(),
        "bearer" | "bearer-token" => "bearer_token".to_string(),
        "session" | "session-auth" | "web-session" | "cookie-session" => "session_auth".to_string(),
        "browser-state" | "browser-profile" | "storage-state" => "browser_state".to_string(),
        "jwt-widget-session" | "widget-session" | "jwt-session" => "jwt_widget_session".to_string(),
        "access-token" => "access_token".to_string(),
        _ => slug.replace('-', "_"),
    }
}

pub(super) fn service_surface_lookup_key(
    service_provider_slug: &str,
    provider_surface_slug: &str,
) -> String {
    format!(
        "{}::{}",
        canonicalize_folder_service_provider_slug(service_provider_slug),
        canonicalize_folder_surface_slug(provider_surface_slug)
    )
}

// Component naming only; filesystem containment belongs to the I/O owners.
pub(super) fn sanitize_file_component(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    normalized
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
