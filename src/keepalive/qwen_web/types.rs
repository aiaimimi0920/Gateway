//! Qwen worker wire messages and refreshed credential material.
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::keepalive) struct QwenWebSessionWorkerInput {
    pub(in crate::keepalive) base_url: String,
    pub(in crate::keepalive) preferred_models: Vec<String>,
    pub(in crate::keepalive) write_credential_file: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) credential_file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) credential_family_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) credential_root_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) browser_executable_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) user_data_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) profile_directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) auth_seed: Option<QwenWebSessionWorkerAuthSeed>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::keepalive) struct QwenWebSessionWorkerAuthSeed {
    pub(in crate::keepalive) email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) password_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::keepalive) struct QwenWebSessionWorkerOutput {
    pub(in crate::keepalive) ok: bool,
    #[serde(default)]
    pub(in crate::keepalive) auth_token: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) expires_at: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) cookie_header: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) _selected_model: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) selected_display_model: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) auth_probe: Option<QwenWebSessionAuthProbe>,
    #[serde(default)]
    pub(in crate::keepalive) credential_file: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) error: Option<QwenWebSessionWorkerError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::keepalive) struct QwenWebSessionAuthProbe {
    #[serde(default)]
    pub(in crate::keepalive) user_id: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) email: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::keepalive) struct QwenWebSessionWorkerError {
    #[serde(default)]
    pub(in crate::keepalive) status: Option<u16>,
    #[serde(default)]
    pub(in crate::keepalive) code: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) message: Option<String>,
}

#[derive(Debug, Clone)]
pub(in crate::keepalive) struct QwenWebRefreshedRuntime {
    pub(in crate::keepalive) api_key: String,
    pub(in crate::keepalive) expires_at: Option<String>,
    pub(in crate::keepalive) cookie_header: Option<String>,
    pub(in crate::keepalive) account_name: Option<String>,
    pub(in crate::keepalive) selected_display_model: Option<String>,
    pub(in crate::keepalive) credential_material_key: Option<String>,
}
