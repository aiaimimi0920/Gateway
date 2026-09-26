use super::*;

/// Returns all built-in presets indexed by ID.
pub fn builtin_presets() -> HashMap<String, ProviderPreset> {
    let mut presets = vec![
        xai_openai_preset(),
        xfyun_preset(),
        xfyun_websocket_preset(),
        accio_preset(),
        perplexity_preset(),
        gemini_business_preset(),
        lumalabs_preset(),
        gemini_canvas_preset(),
        gemini_canvas_chat_preset(),
        freebuff_preset(),
        producer_preset(),
        suno_preset(),
        udio_preset(),
    ];
    #[cfg(feature = "line-chataibot-web-reverse")]
    {
        presets.push(chataibot_preset());
    }
    #[cfg(feature = "line-grok-web-reverse-api")]
    {
        presets.push(grok_preset());
    }
    #[cfg(feature = "line-perplexity-search-official-vendor-api")]
    {
        presets.push(perplexity_search_preset());
    }
    #[cfg(feature = "line-linkup-search-official-vendor-api")]
    {
        presets.push(linkup_preset());
    }
    #[cfg(feature = "line-tavily-search-official-vendor-api")]
    {
        presets.push(tavily_preset());
    }
    #[cfg(feature = "line-you-search-official-vendor-api")]
    {
        presets.push(you_preset());
    }
    #[cfg(feature = "line-exa-search-official-vendor-api")]
    {
        presets.push(exa_preset());
    }
    #[cfg(feature = "line-jina-search-official-vendor-api")]
    {
        presets.push(jina_search_preset());
    }
    #[cfg(feature = "line-jina-reader-official-vendor-api")]
    {
        presets.push(jina_reader_preset());
    }
    #[cfg(feature = "line-websearchapi-search-official-vendor-api")]
    {
        presets.push(websearchapi_preset());
    }
    #[cfg(feature = "line-kiro-official-vendor-api")]
    {
        presets.push(kiro_preset());
    }
    #[cfg(feature = "line-azure-openai-official-vendor-api")]
    {
        presets.push(azure_openai_preset());
    }
    #[cfg(feature = "line-anthropic-messages-official-model-api")]
    {
        presets.push(anthropic_preset());
    }
    #[cfg(feature = "line-aws-bedrock-converse-official-model-api")]
    {
        presets.push(bedrock_converse_preset());
    }
    #[cfg(feature = "line-chatgpt-official-api")]
    {
        presets.push(openai_preset());
    }
    #[cfg(feature = "line-chatgpt-codex-oauth-official")]
    {
        presets.push(codex_preset());
        presets.push(chatgpt_codex_oauth_official_api_preset());
    }
    #[cfg(feature = "line-chatgpt-web-reverse")]
    {
        presets.push(chatgpt_web_reverse_preset());
    }
    #[cfg(feature = "line-cohere-chat-official-model-api")]
    {
        presets.push(cohere_chat_preset());
    }
    #[cfg(feature = "line-groq-openai-official-vendor-api")]
    {
        presets.push(groq_openai_preset());
    }
    #[cfg(feature = "line-nvidia-openai-official-vendor-api")]
    {
        presets.push(nvidia_openai_preset());
    }
    #[cfg(feature = "line-together-openai-aggregator-api")]
    {
        presets.push(together_openai_preset());
    }
    #[cfg(feature = "line-openrouter-openai-aggregator-api")]
    {
        presets.push(openrouter_openai_preset());
    }
    #[cfg(feature = "line-muyuan-openai-aggregator-api")]
    {
        presets.push(muyuan_openai_preset());
    }
    #[cfg(feature = "line-poe-openai-aggregator-api")]
    {
        presets.push(poe_openai_preset());
    }
    #[cfg(feature = "line-longcat-openai-official-model-api")]
    {
        presets.push(longcat_openai_preset());
    }
    #[cfg(feature = "line-deepseek-openai-official-model-api")]
    {
        presets.push(deepseek_openai_preset());
    }
    #[cfg(feature = "line-mistral-openai-official-model-api")]
    {
        presets.push(mistral_openai_preset());
    }
    #[cfg(feature = "line-qwen-official-api")]
    {
        presets.push(qwen_preset());
        presets.push(qwen_dashscope_openai_preset());
        presets.push(qwen_coding_plan_openai_preset());
        presets.push(qwen_coding_plan_anthropic_preset());
    }
    #[cfg(feature = "line-qwen-web-reverse")]
    {
        presets.push(qwen_web_chat_preset());
    }
    #[cfg(feature = "line-aistudio-official")]
    {
        presets.push(gemini_api_preset());
        presets.push(aistudio_official_api_preset());
        presets.push(gemini_api_modular_preset());
    }
    #[cfg(feature = "line-google-agent-platform-official")]
    {
        presets.push(google_agent_platform_preset());
        presets.push(google_agent_platform_official_api_preset());
        presets.push(vertex_gemini_preset());
        presets.push(vertex_official_api_preset());
    }
    #[cfg(feature = "line-gemini-web-reverse")]
    {
        presets.push(gemini_web_chat_preset());
        presets.push(gemini_web_chat_modular_preset());
    }
    #[cfg(feature = "line-aistudio-web-reverse")]
    {
        presets.push(aistudio_web_reverse_preset());
    }
    #[cfg(feature = "line-gemini-canvas-program")]
    {
        presets.push(gemini_canvas_browser_relay_preset());
        presets.push(gemini_canvas_program_relay_preset());
    }
    presets.into_iter().map(|p| (p.id.clone(), p)).collect()
}

/// Look up a preset by ID from the built-in registry.
pub fn get_builtin_preset(id: &str) -> Option<ProviderPreset> {
    let canonical_id = match id.trim() {
        "qwen-web" | "qwen-webui" | "qwen-webui-replay" | "qwen-webui-replay-live" => {
            "qwen-web-chat"
        }
        other => other,
    };
    builtin_presets().get(canonical_id).cloned()
}
