use super::*;

// ── needs_tool_injection ────────────────────────────────────────────

#[test]
fn native_openai_models_no_injection() {
    assert!(!needs_tool_injection("gpt-4o", "openai_compatible"));
    assert!(!needs_tool_injection("gpt-3.5-turbo", "openai_compatible"));
    assert!(!needs_tool_injection("GPT-4o", "openai_compatible"));
}

#[test]
fn native_reasoning_models_no_injection() {
    assert!(!needs_tool_injection("o1-preview", "openai_compatible"));
    assert!(!needs_tool_injection("o3-mini", "openai_compatible"));
    assert!(!needs_tool_injection("o4-mini", "openai_compatible"));
    assert!(!needs_tool_injection("o1", "openai_compatible"));
    assert!(!needs_tool_injection("o3", "openai_compatible"));
}

#[test]
fn native_claude_models_no_injection() {
    assert!(!needs_tool_injection(
        "claude-3-5-sonnet",
        "openai_compatible"
    ));
    assert!(!needs_tool_injection(
        "claude-sonnet-4-6",
        "openai_compatible"
    ));
}

#[test]
fn anthropic_adapter_no_injection() {
    assert!(!needs_tool_injection("any-model", "anthropic_compatible"));
    assert!(!needs_tool_injection(
        "deepseek-chat",
        "anthropic_compatible"
    ));
}

#[test]
fn gemini_and_native_protocol_adapters_never_inject() {
    assert!(!needs_tool_injection(
        "google-gemini-api-fixture",
        "gemini_api_compatible"
    ));
    assert!(!needs_tool_injection(
        "google-gemini-api-fixture",
        "gemini_api_modular_compatible"
    ));
    assert!(!needs_tool_injection(
        "aistudio-web-reverse-fixture",
        "aistudio_web_reverse_compatible"
    ));
    assert!(!needs_tool_injection(
        "gemini-2.5-flash",
        "gemini_canvas_compatible"
    ));
    assert!(!needs_tool_injection(
        "gemini-2.5-flash",
        "gemini_canvas_web_reverse_compatible"
    ));
    assert!(!needs_tool_injection("command-r-plus", "cohere_compatible"));
    assert!(!needs_tool_injection(
        "claude-bedrock",
        "bedrock_converse_compatible"
    ));
}

#[test]
fn deepseek_needs_injection() {
    assert!(needs_tool_injection("deepseek-chat", "openai_compatible"));
    assert!(needs_tool_injection("deepseek-coder", "openai_compatible"));
    assert!(needs_tool_injection("DeepSeek-V2", "openai_compatible"));
}

#[test]
fn unknown_model_needs_injection() {
    assert!(needs_tool_injection("my-custom-model", "openai_compatible"));
    assert!(needs_tool_injection("llama-3-70b", "openai_compatible"));
    assert!(needs_tool_injection("qwen-72b", "openai_compatible"));
}
