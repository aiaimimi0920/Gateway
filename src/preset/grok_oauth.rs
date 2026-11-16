//! A distinct Build product; never aliases the xAI API-Key or grok.com Cookie presets.
use super::ProviderPreset;
pub fn grok_oauth_preset() -> ProviderPreset {
    ProviderPreset {
        id: crate::protocol::grok_oauth::business::PRESET_ID.into(),
        auth_mode: Some(crate::protocol::grok_oauth::business::AUTH_MODE.into()),
        default_model: None,
        responses_path: Some("/responses".into()),
        execution_mode: Some(crate::routing::candidate::ProviderExecutionMode::DirectHttp),
        ..super::openai_preset()
    }
}
