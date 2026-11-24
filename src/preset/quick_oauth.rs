//! Amazon Quick (QuickWork) social OAuth preset.
//! Accounts must bind explicit IAM Identity Center OAuth credentials; no shared provider key exists.

use super::{openai_preset, ProviderPreset};

pub fn quick_oauth_preset() -> ProviderPreset {
    let mut preset = openai_preset();
    preset.id = crate::protocol::quick_oauth::PRESET_ID.into();
    preset.default_model = None;
    preset.auth_mode = Some(crate::protocol::quick_oauth::AUTH_MODE.into());
    preset
}
