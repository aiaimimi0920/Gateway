//! Wire messages shared by the ChatGPT session worker and its caller.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::keepalive) struct ChatGptWebSessionWorkerInput {
    pub(in crate::keepalive) base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) models_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) auth_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) auth_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) cookie_header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) language_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) chatgpt_pow_sources: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) chatgpt_pow_data_build: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) client_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) client_build_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) mailbox_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) mailbox_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) proxy_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) proxy_bypass: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) registration_ip_country: Option<String>,
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
    pub(in crate::keepalive) auth_seed: Option<ChatGptWebSessionWorkerAuthSeed>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) relay_request: Option<ChatGptWebSessionWorkerRelayRequest>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::keepalive) struct ChatGptWebSessionWorkerAuthSeed {
    pub(in crate::keepalive) email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::keepalive) password_sha256: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::keepalive) struct ChatGptWebSessionWorkerRelayRequest {
    pub(in crate::keepalive) stream: bool,
    pub(in crate::keepalive) body: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::keepalive) struct ChatGptWebSessionWorkerOutput {
    pub(in crate::keepalive) ok: bool,
    #[serde(default)]
    pub(in crate::keepalive) auth_token: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) expires_at: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) cookie_header: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) device_id: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) session_id: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) client_version: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) client_build_number: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) user_agent: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) language: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) language_code: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) timezone: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) chatgpt_pow_sources: Option<Vec<String>>,
    #[serde(default)]
    pub(in crate::keepalive) chatgpt_pow_data_build: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) account_name: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) credential_material_key: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) auth_probe: Option<ChatGptWebSessionAuthProbe>,
    #[serde(default)]
    pub(in crate::keepalive) credential_file: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) relay_response: Option<ChatGptWebSessionWorkerRelayResponse>,
    #[serde(default)]
    pub(in crate::keepalive) error: Option<ChatGptWebSessionWorkerError>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::keepalive) struct ChatGptWebSessionWorkerRelayResponse {
    #[serde(default)]
    pub(in crate::keepalive) requirements_status: Option<u16>,
    #[serde(default)]
    pub(in crate::keepalive) requirements_content_type: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) requirements_preview: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) status: Option<u16>,
    #[serde(default)]
    pub(in crate::keepalive) content_type: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) body_text: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) body_preview: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::keepalive) struct ChatGptWebSessionAuthProbe {
    #[serde(default)]
    pub(in crate::keepalive) email: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::keepalive) struct ChatGptWebSessionWorkerError {
    #[serde(default)]
    pub(in crate::keepalive) status: Option<u16>,
    #[serde(default)]
    pub(in crate::keepalive) code: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) message: Option<String>,
}

#[derive(Debug, Clone)]
pub(in crate::keepalive) struct ChatGptWebRefreshedRuntime {
    pub(in crate::keepalive) api_key: String,
    pub(in crate::keepalive) expires_at: Option<String>,
    pub(in crate::keepalive) refresh_token: Option<String>,
    pub(in crate::keepalive) id_token: Option<String>,
    pub(in crate::keepalive) cookie_header: Option<String>,
    pub(in crate::keepalive) device_id: Option<String>,
    pub(in crate::keepalive) session_id: Option<String>,
    pub(in crate::keepalive) client_version: Option<String>,
    pub(in crate::keepalive) client_build_number: Option<String>,
    pub(in crate::keepalive) user_agent: Option<String>,
    pub(in crate::keepalive) language: Option<String>,
    pub(in crate::keepalive) language_code: Option<String>,
    pub(in crate::keepalive) timezone: Option<String>,
    pub(in crate::keepalive) chatgpt_pow_sources: Option<Vec<String>>,
    pub(in crate::keepalive) chatgpt_pow_data_build: Option<String>,
    pub(in crate::keepalive) account_name: Option<String>,
    pub(in crate::keepalive) credential_material_key: Option<String>,
}

#[derive(Debug, Clone)]
pub(in crate::keepalive) struct ChatGptWebSessionWorkerExecution {
    pub(in crate::keepalive) refreshed: ChatGptWebRefreshedRuntime,
    pub(in crate::keepalive) relay_response: Option<ChatGptWebSessionWorkerRelayResponse>,
}
