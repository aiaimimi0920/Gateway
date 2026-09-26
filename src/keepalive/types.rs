//! Wire contracts for keepalive requests and runtime material responses.

use crate::credential_runtime::SessionAuthConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayKeepaliveEnsureRequest {
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "project_id")]
    pub(in crate::keepalive) project_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "session_key"
    )]
    pub(in crate::keepalive) session_key: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "previous_response_id"
    )]
    pub(in crate::keepalive) previous_response_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "credential_id"
    )]
    pub(in crate::keepalive) credential_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "account_name"
    )]
    pub(in crate::keepalive) account_name: Option<String>,
    pub(in crate::keepalive) provider_account_id: String,
    pub(in crate::keepalive) adapter: String,
    pub(in crate::keepalive) base_url: String,
    pub(in crate::keepalive) model: String,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "api_key")]
    pub(in crate::keepalive) api_key: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub(in crate::keepalive) headers: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "extra_body")]
    pub(in crate::keepalive) extra_body: Option<HashMap<String, Value>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "session_auth"
    )]
    pub(in crate::keepalive) session_auth: Option<SessionAuthConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "expires_at")]
    pub(in crate::keepalive) expires_at: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "runtime_state_object_key"
    )]
    pub(in crate::keepalive) runtime_state_object_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayKeepaliveEnsureResponse {
    pub(in crate::keepalive) ready: bool,
    #[serde(default)]
    pub(in crate::keepalive) message: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) api_key: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub(in crate::keepalive) extra_body: Option<HashMap<String, Value>>,
    #[serde(default)]
    pub(in crate::keepalive) session_auth: Option<SessionAuthConfig>,
    #[serde(default)]
    pub(in crate::keepalive) keepalive: Option<crate::credential_runtime::KeepaliveConfig>,
    #[serde(default)]
    pub(in crate::keepalive) expires_at: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) runtime_state_object_key: Option<String>,
    #[serde(default)]
    pub(in crate::keepalive) upstream_session_id: Option<String>,
}
