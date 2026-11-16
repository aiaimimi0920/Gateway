//! Sites share encoders, not accounts, issuers or subscription pools.
use super::{openai_preset, ProviderPreset};

pub fn dim_oauth_preset(global: bool) -> ProviderPreset {
    let mut preset = openai_preset();
    preset.id = if global {
        "dim-global-oauth"
    } else {
        "dim-cn-oauth"
    }
    .into();
    preset.default_model = None;
    preset.auth_mode = Some(crate::protocol::dim_oauth::business::AUTH_MODE.into());
    preset
}
