use super::input::resolve_protocol_profile;

#[test]
fn resolve_protocol_profile_reconciles_legacy_openai_with_codex_backend_url() {
    assert_eq!(
        resolve_protocol_profile(
            Some("openai"),
            "openai_compatible",
            None,
            Some("https://chatgpt.com/backend-api/codex"),
        ),
        "chatgpt_codex_backend"
    );
}

#[test]
fn resolve_protocol_profile_reconciles_legacy_codex_with_official_api_url() {
    assert_eq!(
        resolve_protocol_profile(
            Some("codex"),
            "openai_compatible",
            None,
            Some("https://api.openai.com/v1"),
        ),
        "chatgpt_official_api"
    );
}
