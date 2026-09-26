//! Credential serialization and runtime metadata accessors.
use crate::credential_runtime::{KeepaliveConfig, SessionAuthConfig};
use crate::routing::candidate::ProviderExecutionMode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    UserOwned,
    PlatformUnlimited,
    PlatformLimited,
    AccountCredential,
}

impl CredentialKind {
    /// Lower number = higher priority during resolution.
    pub(super) fn priority(&self) -> u8 {
        match self {
            CredentialKind::UserOwned => 0,
            CredentialKind::AccountCredential => 1,
            CredentialKind::PlatformUnlimited => 2,
            CredentialKind::PlatformLimited => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialEntry {
    pub id: String,
    pub kind: CredentialKind,
    pub project_id: String,
    pub user_id: String,
    pub provider: String,

    // Credential payload
    pub api_key: Option<String>,
    pub api_base_url: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub account_payload: Option<Value>,

    // Quota (platform-limited only)
    pub quota_total_tokens: Option<u64>,
    pub quota_remaining_tokens: Option<u64>,

    // Metadata
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl CredentialEntry {
    /// Extract supported model list from account_payload.
    pub fn supported_models(&self) -> Vec<String> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("supported_models"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Extract preset name from account_payload.
    pub fn preset_name(&self) -> Option<&str> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("preset"))
            .and_then(|v| v.as_str())
    }

    /// Extract extra_body from account_payload.
    pub fn extra_body_fields(&self) -> HashMap<String, Value> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("extra_body").or_else(|| p.get("extraBody")))
            .and_then(|v| serde_json::from_value::<HashMap<String, Value>>(v.clone()).ok())
            .unwrap_or_default()
    }

    /// Extract session-backed auth metadata from account_payload.
    pub fn session_auth(&self) -> Option<SessionAuthConfig> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("session_auth").or_else(|| p.get("sessionAuth")))
            .and_then(|v| serde_json::from_value::<SessionAuthConfig>(v.clone()).ok())
    }

    /// Extract keepalive steward metadata from account_payload.
    pub fn keepalive_config(&self) -> Option<KeepaliveConfig> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("keepalive"))
            .and_then(|v| serde_json::from_value::<KeepaliveConfig>(v.clone()).ok())
    }

    /// Extract runtime_state_object_key from account_payload.
    pub fn runtime_state_object_key(&self) -> Option<&str> {
        self.account_payload
            .as_ref()
            .and_then(|p| {
                p.get("runtime_state_object_key")
                    .or_else(|| p.get("runtimeStateObjectKey"))
            })
            .and_then(|v| v.as_str())
    }

    /// Extract optional human-readable account name from account_payload.
    pub fn account_name(&self) -> Option<&str> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("account_name").or_else(|| p.get("accountName")))
            .and_then(|v| v.as_str())
    }

    /// Extract provider execution_mode from account_payload.
    pub fn execution_mode(&self) -> Option<ProviderExecutionMode> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("execution_mode").or_else(|| p.get("executionMode")))
            .and_then(|v| serde_json::from_value::<ProviderExecutionMode>(v.clone()).ok())
    }

    /// Extract endpoint execution-mode overrides from account_payload.
    pub fn endpoint_execution_modes(&self) -> Option<HashMap<String, ProviderExecutionMode>> {
        self.account_payload
            .as_ref()
            .and_then(|p| {
                p.get("endpoint_execution_modes")
                    .or_else(|| p.get("endpointExecutionModes"))
            })
            .and_then(|v| {
                serde_json::from_value::<HashMap<String, ProviderExecutionMode>>(v.clone()).ok()
            })
    }

    /// Check if this credential can serve a given model.
    /// An empty `supported_models` list means the credential supports any model.
    /// Glob patterns with trailing `*` are supported (prefix matching).
    pub fn supports_model(&self, model: &str) -> bool {
        let models = self.supported_models();
        if models.is_empty() {
            return true; // empty = supports any model
        }
        models.iter().any(|m| {
            if m.ends_with('*') {
                model.starts_with(m.trim_end_matches('*'))
            } else {
                m == model
            }
        })
    }
}
