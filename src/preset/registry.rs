use super::*;

/// Returns all built-in presets indexed by ID.
pub fn builtin_presets() -> HashMap<String, ProviderPreset> {
    let mut presets = vec![
        antigravity_oauth_preset(),
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
        flow_web_preset(),
        stitch_web_preset(),
        suno_preset(),
        udio_preset(),
    ];
    #[cfg(feature = "family-openai-compatible-official-api")]
    presets.push(grok_oauth_preset());
    #[cfg(feature = "line-chataibot-web-reverse")]
    {
        presets.push(chataibot_preset());
    }
    #[cfg(feature = "line-grok-web-reverse-api")]
    {
        presets.push(grok_preset());
    }
    #[cfg(feature = "line-perplexity-web-reverse-api")]
    presets.push(perplexity_web_preset());
    #[cfg(feature = "line-lumalabs-agents-official-api")]
    presets.push(lumalabs_agents_api_preset());
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
    #[cfg(all(
        feature = "line-jina-search-official-vendor-api",
        feature = "line-jina-reader-official-vendor-api"
    ))]
    {
        presets.push(jina_preset());
    }
    #[cfg(feature = "line-websearchapi-search-official-vendor-api")]
    {
        presets.push(websearchapi_preset());
    }
    #[cfg(feature = "line-kiro-official-vendor-api")]
    {
        presets.push(kiro_preset());
        presets.push(kiro_api_key_preset());
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
    #[cfg(feature = "line-river-official-api")]
    presets.push(river_api_preset());
    #[cfg(feature = "line-zenmux-official-api")]
    presets.push(super::openai::zenmux_api_preset());
    #[cfg(feature = "line-agnes-official-api")]
    presets.push(agnes_api_preset());
    #[cfg(feature = "line-lumalabs-inference-official-api")]
    presets.push(lumalabs_inference_api_preset());
    #[cfg(feature = "line-sensenova-token-plan")]
    presets.push(sensenova_token_plan_preset());
    #[cfg(feature = "line-commandcode-provider-api")]
    presets.push(commandcode_api_preset());
    #[cfg(feature = "line-sap-aicore-chat")]
    presets.push(sap_aicore_chat_preset());
    #[cfg(feature = "line-minimax-m-plan")]
    presets.extend([minimax_m_plan_preset(false), minimax_m_plan_preset(true)]);
    #[cfg(feature = "line-cloudflare-workers-ai")]
    presets.push(cloudflare_workers_ai_preset());
    #[cfg(feature = "line-dim-user-oauth")]
    presets.extend([dim_oauth_preset(false), dim_oauth_preset(true)]);
    #[cfg(feature = "line-nous-portal-oauth")]
    presets.push(nous_portal_oauth_preset());
    #[cfg(feature = "line-quick-oauth")]
    presets.push(quick_oauth_preset());
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
    #[cfg(feature = "line-kilo-free")]
    presets.push(super::openai::kilo_free_preset());
    #[cfg(feature = "family-openai-compatible-official-api")]
    presets.push(super::openai::vercel_free_preset());
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
        presets.push(super::openai::mistral_studio_preset());
        presets.push(super::openai::mistral_vibe_preset());
    }
    #[cfg(feature = "line-qwen-official-api")]
    {
        presets.push(qwen_official_api_preset());
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
        presets.push(vertex_express_api_preset());
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
