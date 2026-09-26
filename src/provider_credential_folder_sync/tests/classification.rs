//! Provider identity and modular surface contracts.
use super::super::canonicalization::{
    canonicalize_folder_family_slug, canonicalize_folder_surface_slug,
};
use super::super::classification::{derive_provider_family_slug, derive_provider_surface_slug};
use super::build_test_provider_account;
use crate::protocol::gemini::shared::{
    GEMINI_API_MODULAR_PROFILE, GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE,
    GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE, GEMINI_WEB_REVERSE_MODULAR_PROFILE,
};

#[test]
fn derive_provider_family_slug_prefers_qwen_web_chat_for_qwen_web_surfaces() {
    let provider = build_test_provider_account(
        "Qwen WebUI Replay Live",
        "qwen_web_compatible",
        "qwen_web_chat",
        "qwen_web_chat",
        "https://chat.qwen.ai",
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    );
    assert_eq!(derive_provider_family_slug(&provider), "qwen-web-chat");
    assert_eq!(
        canonicalize_folder_family_slug("qwen_web_chat"),
        "qwen-web-chat"
    );
    assert_eq!(canonicalize_folder_family_slug("qwen-web"), "qwen-web-chat");
    assert_eq!(
        canonicalize_folder_family_slug("qwen-webui"),
        "qwen-web-chat"
    );
    assert_eq!(
        canonicalize_folder_family_slug("qwen-webui-replay-live"),
        "qwen-web-chat"
    );
}

#[test]
fn derive_provider_family_slug_prefers_accio_for_phoenix_surfaces() {
    let provider = build_test_provider_account(
        "Accio Live",
        "accio_compatible",
        "accio",
        "openai_responses",
        "https://phoenix-gw.alibaba.com",
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    );
    assert_eq!(derive_provider_family_slug(&provider), "accio");
    assert_eq!(canonicalize_folder_family_slug("accio-manager"), "accio");
}

#[test]
fn derive_provider_family_slug_prefers_nvidia_for_official_nim_surfaces() {
    let provider = build_test_provider_account(
        "NVIDIA Platform Live",
        "openai_compatible",
        "nvidia",
        "openai",
        "https://integrate.api.nvidia.com",
        Some("official_model_api"),
        None,
    );
    assert_eq!(derive_provider_family_slug(&provider), "nvidia");
    assert_eq!(canonicalize_folder_family_slug("nvidia-platform"), "nvidia");
    assert_eq!(canonicalize_folder_family_slug("nvidia_nim"), "nvidia");
}

#[test]
fn derive_provider_surface_slug_maps_chatgpt_platform_dual_lines() {
    let mut official = build_test_provider_account(
        "OpenAI Platform",
        "openai_compatible",
        "openai",
        "chatgpt_official_api",
        "https://api.openai.com/v1",
        Some("official_vendor_api"),
        None,
    );
    official.service_provider_key = "chatgpt_platform".to_string();
    official.service_provider_label = "ChatGPT Platform".to_string();
    assert_eq!(
        derive_provider_surface_slug(&official),
        "chatgpt-official-api"
    );
    assert_eq!(
        canonicalize_folder_surface_slug("openai-platform"),
        "chatgpt-official-api"
    );

    let mut codex = build_test_provider_account(
        "ChatGPT Codex Backend",
        "openai_compatible",
        "openai",
        "chatgpt_codex_backend",
        "https://chatgpt.com/backend-api/codex",
        Some("official_vendor_api"),
        None,
    );
    codex.service_provider_key = "chatgpt_platform".to_string();
    codex.service_provider_label = "ChatGPT Platform".to_string();
    assert_eq!(
        derive_provider_surface_slug(&codex),
        "chatgpt-codex-backend"
    );
    assert_eq!(
        canonicalize_folder_surface_slug("chatgpt-codex-backend"),
        "chatgpt-codex-backend"
    );
}

#[test]
fn derive_provider_family_slug_keeps_gemini_modular_lines_explicit() {
    let mut official = build_test_provider_account(
        "Gemini API Modular",
        "gemini_api_modular_compatible",
        "gemini_generate_content",
        GEMINI_API_MODULAR_PROFILE,
        "https://example.invalid",
        Some("official_model_api"),
        None,
    );
    official.service_provider_key = "gemini_platform".to_string();
    official.service_provider_label = "Gemini Platform".to_string();
    assert_eq!(derive_provider_family_slug(&official), "gemini");

    let mut web = build_test_provider_account(
        "Gemini Web Reverse Modular",
        "gemini_web_reverse_modular_compatible",
        "gemini_web_chat",
        GEMINI_WEB_REVERSE_MODULAR_PROFILE,
        "https://example.invalid",
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    );
    web.service_provider_key = "gemini_platform".to_string();
    web.service_provider_label = "Gemini Platform".to_string();
    assert_eq!(derive_provider_family_slug(&web), "gemini-web-chat");

    let mut program = build_test_provider_account(
        "Gemini Canvas Program Relay",
        "gemini_canvas_program_web_reverse_compatible",
        "gemini_canvas_images",
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE,
        "https://example.invalid",
        Some("web_reverse_api"),
        Some("browser_backed"),
    );
    program.service_provider_key = "gemini_platform".to_string();
    program.service_provider_label = "Gemini Platform".to_string();
    assert_eq!(derive_provider_family_slug(&program), "gemini");
}

#[test]
fn derive_provider_surface_slug_maps_gemini_modular_lines() {
    let mut official = build_test_provider_account(
        "Gemini API Modular",
        "gemini_api_modular_compatible",
        "gemini_generate_content",
        GEMINI_API_MODULAR_PROFILE,
        "https://example.invalid",
        Some("official_model_api"),
        None,
    );
    official.service_provider_key = "gemini_platform".to_string();
    official.service_provider_label = "Gemini Platform".to_string();
    assert_eq!(
        derive_provider_surface_slug(&official),
        "google-gemini-api-modular"
    );

    let mut browser = build_test_provider_account(
        "Gemini Canvas Browser Relay",
        "gemini_canvas_web_reverse_compatible",
        "gemini_canvas_images",
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE,
        "https://example.invalid",
        Some("web_reverse_api"),
        Some("browser_backed"),
    );
    browser.service_provider_key = "gemini_platform".to_string();
    browser.service_provider_label = "Gemini Platform".to_string();
    assert_eq!(
        derive_provider_surface_slug(&browser),
        "gemini-canvas-browser-relay"
    );

    let mut program = build_test_provider_account(
        "Gemini Canvas Program Relay",
        "gemini_canvas_program_web_reverse_compatible",
        "gemini_canvas_images",
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE,
        "https://example.invalid",
        Some("web_reverse_api"),
        Some("browser_backed"),
    );
    program.service_provider_key = "gemini_platform".to_string();
    program.service_provider_label = "Gemini Platform".to_string();
    assert_eq!(
        derive_provider_surface_slug(&program),
        "gemini-canvas-program-relay"
    );
}

#[test]
fn derive_provider_surface_slug_maps_chataibot_image_line() {
    let mut provider = build_test_provider_account(
        "ChatAIBot Images",
        "chataibot_compatible",
        "chataibot_images",
        "chataibot",
        "https://chataibot.pro",
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    );
    provider.service_provider_key = "chataibot_platform".to_string();
    provider.service_provider_label = "ChatAIBot".to_string();

    assert_eq!(derive_provider_surface_slug(&provider), "chataibot-images");
    assert_eq!(
        canonicalize_folder_surface_slug("chataibot-images"),
        "chataibot-images"
    );
}
