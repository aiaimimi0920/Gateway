use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const AISTUDIO_TARGET_RPC_CONTRACT_FILE_NAME: &str = "aistudio-target-rpc-contract.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AIStudioTargetRpcEndpointContract {
    pub url: Option<String>,
    #[serde(default)]
    pub request_headers: HashMap<String, String>,
    pub request_body_preview: Option<String>,
    pub response_status: Option<u16>,
    #[serde(default)]
    pub response_headers: HashMap<String, String>,
    pub response_body_preview: Option<String>,
}

impl AIStudioTargetRpcEndpointContract {
    pub fn request_header(&self, name: &str) -> Option<&str> {
        self.request_headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AIStudioTargetRpcContract {
    #[serde(default)]
    pub captured_target_rpc_contract: bool,
    pub app_id: Option<String>,
    pub model_path: Option<String>,
    pub prompt_text: Option<String>,
    pub generation_id: Option<String>,
    pub code_assistant_opaque_token: Option<String>,
    pub final_text: Option<String>,
    #[serde(default)]
    pub code_assistant_offline: AIStudioTargetRpcEndpointContract,
    #[serde(default)]
    pub stream_code_assistant_offline_generation: AIStudioTargetRpcEndpointContract,
}

impl AIStudioTargetRpcContract {
    pub fn preferred_request_url(&self) -> Option<&str> {
        self.code_assistant_offline
            .url
            .as_deref()
            .or(self.stream_code_assistant_offline_generation.url.as_deref())
    }

    pub fn preferred_request_header(&self, name: &str) -> Option<&str> {
        self.code_assistant_offline
            .request_header(name)
            .or_else(|| {
                self.stream_code_assistant_offline_generation
                    .request_header(name)
            })
    }

    pub fn preferred_model_name(&self) -> Option<&str> {
        let raw = self.model_path.as_deref()?.trim();
        if raw.is_empty() {
            return None;
        }
        if let Some(stripped) = raw.strip_prefix("models/") {
            let stripped = stripped.trim();
            return (!stripped.is_empty()).then_some(stripped);
        }
        Some(raw)
    }
}

pub fn target_rpc_contract_object_key(runtime_state_object_key: &str) -> String {
    let trimmed = runtime_state_object_key.trim();
    if let Some(prefix) = trimmed.strip_suffix("/storage-state.json") {
        return format!("{prefix}/{AISTUDIO_TARGET_RPC_CONTRACT_FILE_NAME}");
    }
    if let Some(prefix) = trimmed.strip_suffix("\\storage-state.json") {
        return format!("{prefix}/{AISTUDIO_TARGET_RPC_CONTRACT_FILE_NAME}");
    }
    format!("{trimmed}.{AISTUDIO_TARGET_RPC_CONTRACT_FILE_NAME}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_rpc_contract_object_key_uses_storage_state_prefix() {
        assert_eq!(
            target_rpc_contract_object_key("credential-runtime/aistudio/demo/storage-state.json"),
            "credential-runtime/aistudio/demo/aistudio-target-rpc-contract.json"
        );
        assert_eq!(
            target_rpc_contract_object_key(
                "credential-runtime\\aistudio\\demo\\storage-state.json"
            ),
            "credential-runtime\\aistudio\\demo/aistudio-target-rpc-contract.json"
        );
    }

    #[test]
    fn preferred_model_name_strips_models_prefix() {
        let contract = AIStudioTargetRpcContract {
            model_path: Some("models/gemini-3-flash-preview".to_string()),
            ..Default::default()
        };
        assert_eq!(
            contract.preferred_model_name(),
            Some("gemini-3-flash-preview")
        );
    }

    #[test]
    fn preferred_request_header_reads_case_insensitive_value() {
        let contract = AIStudioTargetRpcContract {
            code_assistant_offline: AIStudioTargetRpcEndpointContract {
                request_headers: HashMap::from([(
                    "x-goog-api-key".to_string(),
                    "AIzaSyFixture".to_string(),
                )]),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            contract.preferred_request_header("X-Goog-Api-Key"),
            Some("AIzaSyFixture")
        );
    }
}
