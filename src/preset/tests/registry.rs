use super::*;

#[cfg(feature = "line-qwen-web-reverse")]
#[test]
fn get_builtin_preset_accepts_qwen_web_historical_aliases() {
    let legacy = get_builtin_preset("qwen-web").expect("legacy alias preset");
    assert_eq!(legacy.id, "qwen-web-chat");

    let webui = get_builtin_preset("qwen-webui").expect("legacy webui alias preset");
    assert_eq!(webui.id, "qwen-web-chat");

    let replay = get_builtin_preset("qwen-webui-replay").expect("historical alias preset");
    assert_eq!(replay.id, "qwen-web-chat");

    let replay_live =
        get_builtin_preset("qwen-webui-replay-live").expect("historical live alias preset");
    assert_eq!(replay_live.id, "qwen-web-chat");
}

#[test]
fn builtin_presets_contains_all() {
    let presets = builtin_presets();
    assert!(presets.contains_key("codex"));
    assert!(presets.contains_key("openai"));
    assert!(presets.contains_key("groq-openai"));
    assert!(presets.contains_key("together-openai"));
    assert!(presets.contains_key("openrouter-openai"));
    assert!(presets.contains_key("deepseek-openai"));
    assert!(presets.contains_key("mistral-openai"));
    assert!(presets.contains_key("xai-openai"));
    assert!(presets.contains_key("nvidia-openai"));
    assert!(presets.contains_key("gemini-api"));
    assert!(presets.contains_key("gemini-api-modular"));
    assert!(presets.contains_key("google-agent-platform"));
    assert!(presets.contains_key("google-agent-platform-official-api"));
    assert!(presets.contains_key("vertex-gemini"));
    assert!(presets.contains_key("gemini-web-chat"));
    assert!(presets.contains_key("gemini-web-chat-modular"));
    assert!(presets.contains_key("bedrock-converse"));
    assert!(presets.contains_key("cohere-chat"));
    assert!(presets.contains_key("xfyun"));
    assert!(presets.contains_key("xfyun-websocket"));
    assert!(presets.contains_key("anthropic"));
    assert!(presets.contains_key("accio"));
    assert!(presets.contains_key("qwen"));
    assert!(presets.contains_key("qwen-dashscope-openai"));
    assert!(presets.contains_key("qwen-coding-plan-openai"));
    assert!(presets.contains_key("qwen-coding-plan-anthropic"));
    assert!(presets.contains_key("qwen-web-chat"));
    assert!(presets.contains_key("grok"));
    assert!(presets.contains_key("perplexity"));
    assert!(presets.contains_key("perplexity-search"));
    assert!(presets.contains_key("linkup"));
    assert!(presets.contains_key("tavily"));
    assert!(presets.contains_key("you"));
    assert!(presets.contains_key("exa"));
    assert!(presets.contains_key("jina-search"));
    assert!(presets.contains_key("jina-reader"));
    assert!(presets.contains_key("websearchapi"));
    assert!(presets.contains_key("gemini-business"));
    assert!(presets.contains_key("chataibot"));
    assert!(presets.contains_key("lumalabs"));
    assert!(presets.contains_key("gemini-canvas"));
    assert!(presets.contains_key("gemini-canvas-browser-relay"));
    assert!(presets.contains_key("gemini-canvas-program-relay"));
    assert!(presets.contains_key("kiro"));
    assert!(presets.contains_key("freebuff"));
    assert!(presets.contains_key("producer"));
}
