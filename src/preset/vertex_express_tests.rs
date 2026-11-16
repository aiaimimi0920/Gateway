//! Product identity must exist before a Key can be saved or sent through the generic Gemini adapter.
#[cfg(feature = "line-google-agent-platform-official")]
#[test]
fn vertex_express_preset_is_not_aistudio_or_full_vertex_oauth() {
    let preset =
        super::get_builtin_preset("vertex-express-api").expect("independent Express preset");
    assert_eq!(preset.adapter, "gemini_api_compatible");
    assert_eq!(preset.auth_mode.as_deref(), Some("vertex_express_api_key"));
    assert_eq!(preset.auth_header_name.as_deref(), Some("x-goog-api-key"));
    assert!(preset.default_model.is_none());
    assert!(preset.session_auth.is_none());
    assert!(preset.headers.is_empty());
}
