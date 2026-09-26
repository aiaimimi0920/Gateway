//! Folder path parsing, material kinds and nested export layout contracts.
use super::super::layout::{
    default_folder_sync_relative_path, describe_folder_import_path, normalize_source_path_key,
};
use super::super::payload_metadata::derive_credential_material_kind;
use super::{build_test_credential, build_test_provider_account};
use crate::protocol::gemini::shared::{
    GEMINI_API_MODULAR_PROFILE, GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE,
    GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE, GEMINI_WEB_REVERSE_MODULAR_PROFILE,
};

#[test]
fn normalize_source_path_key_supports_windows_separators() {
    assert_eq!(
        normalize_source_path_key(r"codex\team-alpha.json").as_deref(),
        Some("codex/team-alpha.json")
    );
}

#[test]
fn default_folder_sync_relative_path_uses_wave3_service_surface_layout() {
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
    let mut azure_credential =
        build_test_credential("cred-azure", "folder_sync", None, "active", None);
    azure_credential.payload = serde_json::json!({"apiKey":"sk-azure"});
    assert_eq!(
        default_folder_sync_relative_path(&azure, &azure_credential),
        "azure-openai-platform/azure-openai/api-key/cred-azure.json"
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
    let mut bedrock_credential =
        build_test_credential("cred-bedrock", "folder_sync", None, "active", None);
    bedrock_credential.payload = serde_json::json!({"authToken":"signed-token"});
    assert_eq!(
        default_folder_sync_relative_path(&bedrock, &bedrock_credential),
        "aws-bedrock-platform/bedrock-converse/bearer-token/cred-bedrock.json"
    );
}

#[test]
fn derive_credential_material_kind_defaults_cover_gemini_modular_profiles() {
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
    assert_eq!(derive_credential_material_kind(&official, None), "api_key");

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
    assert_eq!(derive_credential_material_kind(&web, None), "session_auth");

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
        derive_credential_material_kind(&browser, None),
        "browser_state"
    );
}

#[test]
fn default_folder_sync_relative_path_uses_modular_gemini_surface_slugs() {
    let mut browser = build_test_provider_account(
        "Gemini Canvas Browser Relay",
        "gemini_canvas_web_reverse_compatible",
        "gemini_canvas_images",
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE,
        "https://gemini.google.com",
        Some("web_reverse_api"),
        Some("browser_backed"),
    );
    browser.service_provider_key = "gemini_platform".to_string();
    browser.service_provider_label = "Gemini Platform".to_string();

    let mut browser_credential =
        build_test_credential("cred-browser", "folder_sync", None, "active", None);
    browser_credential.payload = serde_json::json!({
        "runtimeStateObjectKey": "credential-runtime/gemini-canvas/browser/storage-state.json"
    });

    assert_eq!(
        default_folder_sync_relative_path(&browser, &browser_credential),
        "gemini-platform/gemini-canvas-browser-relay/browser-state/cred-browser.json"
    );

    let mut program = build_test_provider_account(
        "Gemini Canvas Program Relay",
        "gemini_canvas_program_web_reverse_compatible",
        "gemini_canvas_images",
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE,
        "https://gemini.google.com",
        Some("web_reverse_api"),
        Some("browser_backed"),
    );
    program.service_provider_key = "gemini_platform".to_string();
    program.service_provider_label = "Gemini Platform".to_string();

    let mut program_credential =
        build_test_credential("cred-program", "folder_sync", None, "active", None);
    program_credential.payload = serde_json::json!({
        "runtimeStateObjectKey": "credential-runtime/gemini-canvas/program/storage-state.json"
    });

    assert_eq!(
        default_folder_sync_relative_path(&program, &program_credential),
        "gemini-platform/gemini-canvas-program-relay/browser-state/cred-program.json"
    );
}

#[test]
fn describe_folder_import_path_supports_nested_service_surface_material_layout() {
    let descriptor =
        describe_folder_import_path("gemini-platform/gemini-canvas-images/browser-state/main.json")
            .expect("descriptor");
    assert_eq!(
        descriptor.service_provider_slug.as_deref(),
        Some("gemini-platform")
    );
    assert_eq!(descriptor.provider_surface_slug, "gemini-canvas-images");
    assert_eq!(
        descriptor.credential_material_kind.as_deref(),
        Some("browser_state")
    );
}

#[test]
fn default_folder_sync_relative_path_uses_nested_rollout_for_qwen_and_gemini() {
    let mut qwen_provider = build_test_provider_account(
        "Qwen DashScope OpenAI",
        "openai_compatible",
        "openai",
        "qwen_dashscope_openai",
        "https://dashscope.aliyuncs.com/compatible-mode/v1",
        Some("official_model_api"),
        None,
    );
    qwen_provider.service_provider_key = "qwen_platform".to_string();
    qwen_provider.service_provider_label = "Qwen Platform".to_string();

    let mut qwen_credential =
        build_test_credential("cred-qwen", "folder_sync", None, "active", None);
    qwen_credential.payload = serde_json::json!({
        "apiKey": "sk-test"
    });

    assert_eq!(
        default_folder_sync_relative_path(&qwen_provider, &qwen_credential),
        "qwen-platform/qwen-dashscope-openai/api-key/cred-qwen.json"
    );

    let mut gemini_provider = build_test_provider_account(
        "Gemini Canvas Images",
        "gemini_canvas_compatible",
        "gemini_canvas_images",
        "gemini_canvas",
        "https://gemini.google.com",
        Some("web_reverse_api"),
        Some("browser_challenge"),
    );
    gemini_provider.service_provider_key = "gemini_platform".to_string();
    gemini_provider.service_provider_label = "Gemini Platform".to_string();

    let mut gemini_credential =
        build_test_credential("cred-gemini", "folder_sync", None, "active", None);
    gemini_credential.payload = serde_json::json!({
        "runtimeStateObjectKey": "credential-runtime/gemini-canvas/main/storage-state.json"
    });

    assert_eq!(
        default_folder_sync_relative_path(&gemini_provider, &gemini_credential),
        "gemini-platform/gemini-canvas-images/browser-state/cred-gemini.json"
    );
}

#[test]
fn default_folder_sync_relative_path_uses_nested_rollout_for_chataibot() {
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

    let mut credential =
        build_test_credential("cred-chataibot", "folder_sync", None, "active", None);
    credential.payload = serde_json::json!({
        "apiKey": "token-123"
    });

    assert_eq!(
        default_folder_sync_relative_path(&provider, &credential),
        "chataibot-platform/chataibot-images/session-auth/cred-chataibot.json"
    );
}

#[test]
fn default_folder_sync_relative_path_covers_all_qwen_canonical_surfaces() {
    let mut dashscope_provider = build_test_provider_account(
        "Qwen DashScope OpenAI",
        "openai_compatible",
        "openai",
        "qwen_dashscope_openai",
        "https://dashscope.aliyuncs.com/compatible-mode/v1",
        Some("official_model_api"),
        None,
    );
    dashscope_provider.service_provider_key = "qwen_platform".to_string();
    dashscope_provider.service_provider_label = "Qwen Platform".to_string();

    let mut dashscope_credential =
        build_test_credential("cred-qwen-dashscope", "folder_sync", None, "active", None);
    dashscope_credential.payload = serde_json::json!({ "apiKey": "sk-test-dashscope" });
    assert_eq!(
        default_folder_sync_relative_path(&dashscope_provider, &dashscope_credential),
        "qwen-platform/qwen-dashscope-openai/api-key/cred-qwen-dashscope.json"
    );

    let mut coding_openai_provider = build_test_provider_account(
        "Qwen Coding Plan OpenAI",
        "openai_compatible",
        "openai",
        "qwen_coding_plan_openai",
        "https://coding.dashscope.aliyuncs.com/v1",
        Some("official_model_api"),
        None,
    );
    coding_openai_provider.service_provider_key = "qwen_platform".to_string();
    coding_openai_provider.service_provider_label = "Qwen Platform".to_string();

    let mut coding_openai_credential = build_test_credential(
        "cred-qwen-coding-openai",
        "folder_sync",
        None,
        "active",
        None,
    );
    coding_openai_credential.payload = serde_json::json!({ "apiKey": "sk-test-coding-openai" });
    assert_eq!(
        default_folder_sync_relative_path(&coding_openai_provider, &coding_openai_credential),
        "qwen-platform/qwen-coding-plan-openai/api-key/cred-qwen-coding-openai.json"
    );

    let mut coding_anthropic_provider = build_test_provider_account(
        "Qwen Coding Plan Anthropic",
        "anthropic_compatible",
        "anthropic",
        "qwen_coding_plan_anthropic",
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
        Some("official_model_api"),
        None,
    );
    coding_anthropic_provider.service_provider_key = "qwen_platform".to_string();
    coding_anthropic_provider.service_provider_label = "Qwen Platform".to_string();

    let mut coding_anthropic_credential = build_test_credential(
        "cred-qwen-coding-anthropic",
        "folder_sync",
        None,
        "active",
        None,
    );
    coding_anthropic_credential.payload =
        serde_json::json!({ "apiKey": "sk-test-coding-anthropic" });
    assert_eq!(
        default_folder_sync_relative_path(&coding_anthropic_provider, &coding_anthropic_credential),
        "qwen-platform/qwen-coding-plan-anthropic/api-key/cred-qwen-coding-anthropic.json"
    );

    let mut web_provider = build_test_provider_account(
        "Qwen WebUI Replay Live",
        "qwen_web_compatible",
        "qwen_web_chat",
        "qwen_web_chat",
        "https://chat.qwen.ai",
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    );
    web_provider.service_provider_key = "qwen_platform".to_string();
    web_provider.service_provider_label = "Qwen Platform".to_string();

    let mut web_credential =
        build_test_credential("cred-qwen-web", "folder_sync", None, "active", None);
    web_credential.payload = serde_json::json!({ "apiKey": "qwen-web-token" });
    assert_eq!(
        default_folder_sync_relative_path(&web_provider, &web_credential),
        "qwen-platform/qwen-web-chat/session-auth/cred-qwen-web.json"
    );
}
