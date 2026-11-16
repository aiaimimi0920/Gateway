//! Portal subscriptions have no bootstrap model, API-key fallback or first-party client identity.
use super::{openai_preset, ProviderPreset};

pub fn nous_portal_oauth_preset() -> ProviderPreset {
    let mut preset = openai_preset();
    preset.id = "nous-portal-oauth".into();
    preset.default_model = None;
    preset.auth_mode = Some(crate::protocol::nous_portal::business::AUTH_MODE.into());
    preset
}
