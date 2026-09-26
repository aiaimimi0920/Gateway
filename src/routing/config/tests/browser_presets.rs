use super::*;

#[test]
fn yaml_parsing_with_producer_preset() {
    let yaml = r#"
providers:
  - id: producer-main
    preset: producer
    base_url: "https://www.flowmusic.app"
    api_key: "producer-session"
    supported_models: ["producer:image", "producer:standard", "producer:music-video"]
model_routes:
  - pattern: "producer:*"
    provider_ids: [producer-main]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let image_candidates = store.resolve_candidates(Some("producer:image"));
    assert_eq!(image_candidates.len(), 1);
    assert_eq!(image_candidates[0].payload.adapter, "producer_compatible");
    let candidates = store.resolve_candidates(Some("producer:standard"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].payload.adapter, "producer_compatible");
    assert_eq!(
        candidates[0]
            .payload
            .session_auth
            .as_ref()
            .map(|cfg| cfg.transport.as_str()),
        Some("bearer")
    );
    let video_candidates = store.resolve_candidates(Some("producer:music-video"));
    assert_eq!(video_candidates.len(), 1);
    assert_eq!(video_candidates[0].payload.adapter, "producer_compatible");
}

#[test]
fn yaml_parsing_with_udio_preset() {
    let yaml = r#"
providers:
  - id: udio-main
    preset: udio
    base_url: "https://www.udio.com"
    api_key: "udio-session"
    supported_models: [udio-music]
model_routes:
  - pattern: "udio-*"
    provider_ids: [udio-main]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(Some("udio-music"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].payload.adapter, "udio_compatible");
    assert_eq!(candidates[0].protocol_family, "udio_music");
    assert_eq!(
        candidates[0]
            .payload
            .session_auth
            .as_ref()
            .map(|cfg| cfg.transport.as_str()),
        Some("cookie")
    );
    assert_eq!(
        candidates[0]
            .payload
            .session_auth
            .as_ref()
            .map(|cfg| cfg.primary_cookie_name()),
        Some("sb-ssr-production-auth-token")
    );
}

#[cfg(feature = "line-qwen-web-reverse")]
#[test]
fn yaml_parsing_accepts_qwen_web_historical_preset_aliases() {
    let yaml = r#"
providers:
  - id: qwen-web-live
    preset: qwen-webui
    base_url: "https://chat.qwen.ai"
    api_key: "qwen-session"
    supported_models: [qwen-web-model]
model_routes:
  - pattern: "qwen-web-model"
    provider_ids: [qwen-web-live]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(Some("qwen-web-model"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].payload.adapter, "qwen_web_compatible");
    assert_eq!(candidates[0].protocol_profile, "qwen_web_chat");
    assert_eq!(candidates[0].protocol_family, "qwen_web_chat");
}

#[test]
fn yaml_parsing_with_gemini_business_preset() {
    let yaml = r#"
providers:
  - id: gemini-business-main
    preset: gemini-business
    base_url: "https://biz-discoveryengine.googleapis.com/v1alpha"
    api_key: "jwt-token"
    supported_models: [nano-banana-pro, gemini-3-pro-image-preview]
    session_auth:
      transport: "bearer"
      header_name: "Authorization"
    extra_body:
      configId: "cfg-123"
      session: "projects/demo/sessions/123"
model_routes:
  - pattern: "nano-banana*"
    provider_ids: [gemini-business-main]
    priority: 10
  - pattern: "gemini-3-pro-image-preview"
    provider_ids: [gemini-business-main]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(Some("nano-banana-pro"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].payload.adapter, "gemini_business_compatible");
    assert_eq!(candidates[0].protocol_family, "gemini_business_images");
    assert_eq!(
        candidates[0].payload.chat_completions_path.as_deref(),
        Some("/locations/global/widgetStreamAssist")
    );
    let extra = candidates[0].payload.extra_body.as_ref().unwrap();
    assert_eq!(extra.get("configId"), Some(&json!("cfg-123")));
    assert_eq!(
        extra.get("session"),
        Some(&json!("projects/demo/sessions/123"))
    );
}

#[test]
fn yaml_parsing_with_gemini_canvas_preset() {
    let yaml = r#"
providers:
  - id: gemini-canvas-main
    preset: gemini-canvas
    base_url: "https://gemini.google.com"
    supported_models: [gemini-2.5-flash-image-preview]
    keepalive:
      service_url: "http://gateway.internal"
      refresh_before_secs: 300
    credentials:
      - id: gemini-canvas-account-1
        api_key: ""
        runtime_state_object_key: "objects/gemini-canvas/auth-1.json"
        account_name: "canvas-main"
model_routes:
  - pattern: "gemini-2.5-flash-image-preview"
    provider_ids: [gemini-canvas-main]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(Some("gemini-2.5-flash-image-preview"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].payload.adapter, "gemini_canvas_compatible");
    assert_eq!(candidates[0].protocol_family, "gemini_canvas_images");
    assert_eq!(
        candidates[0].payload.runtime_state_object_key.as_deref(),
        Some("objects/gemini-canvas/auth-1.json")
    );
    assert_eq!(
        candidates[0].payload.account_name.as_deref(),
        Some("canvas-main")
    );
    assert_eq!(
        candidates[0]
            .payload
            .extra_body
            .as_ref()
            .and_then(|extra| extra.get("shareId")),
        Some(&json!(
            crate::protocol::gemini_canvas::GEMINI_CANVAS_DEFAULT_SHARE_ID
        ))
    );
}

#[test]
fn yaml_parsing_with_gemini_canvas_browser_relay_preset() {
    let yaml = r#"
providers:
  - id: gemini-canvas-relay
    preset: gemini-canvas-browser-relay
    base_url: "https://gemini.google.com"
    supported_models: [gemini-3-flash-preview]
    credentials:
      - id: gemini-canvas-relay-account-1
        api_key: ""
        runtime_state_object_key: "objects/gemini-canvas/relay-auth-1.json"
        account_name: "canvas-relay"
model_routes:
  - pattern: "gemini-3-flash-preview"
    provider_ids: [gemini-canvas-relay]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(Some("gemini-3-flash-preview"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].payload.adapter,
        "gemini_canvas_web_reverse_compatible"
    );
    assert_eq!(
        candidates[0].payload.execution_mode,
        Some(ProviderExecutionMode::BrowserBacked)
    );
    assert_eq!(
        candidates[0].payload.runtime_state_object_key.as_deref(),
        Some("objects/gemini-canvas/relay-auth-1.json")
    );
}
