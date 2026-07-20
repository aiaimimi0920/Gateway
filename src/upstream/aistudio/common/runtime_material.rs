use std::time::Duration;

use crate::error::GatewayError;
use crate::protocol::aistudio_web;
use crate::upstream::aistudio::common::capture_contract::target_rpc_contract_object_key;
use crate::upstream::aistudio::common::types::AIStudioBrowserWorkerInput;

#[derive(Debug, Clone)]
pub struct AIStudioWebReverseRuntimeMaterial {
    pub runtime_state_object_key: Option<String>,
    pub target_rpc_contract_object_key: Option<String>,
    pub app_url: String,
    pub browser_executable_path: Option<String>,
    pub cloud_api_key: Option<String>,
}

impl From<&aistudio_web::AIStudioWebConfig> for AIStudioWebReverseRuntimeMaterial {
    fn from(value: &aistudio_web::AIStudioWebConfig) -> Self {
        Self {
            runtime_state_object_key: value.runtime_state_object_key.clone(),
            target_rpc_contract_object_key: value.target_rpc_contract_object_key.clone(),
            app_url: value.app_url.clone(),
            browser_executable_path: value.browser_executable_path.clone(),
            cloud_api_key: value.cloud_api_key.clone(),
        }
    }
}

impl AIStudioWebReverseRuntimeMaterial {
    pub fn require_runtime_state_object_key(&self) -> Result<&str, GatewayError> {
        self.runtime_state_object_key.as_deref().ok_or_else(|| {
            GatewayError::bad_request(
                "AI Studio Web reverse browser execution requires runtimeStateObjectKey.",
            )
            .with_code("missing_aistudio_runtime_state_object_key")
        })
    }

    pub fn require_cloud_api_key(&self, message: &str, code: &str) -> Result<&str, GatewayError> {
        self.cloud_api_key
            .as_deref()
            .ok_or_else(|| GatewayError::bad_request(message).with_code(code))
    }

    pub fn resolved_target_rpc_contract_object_key(&self) -> Option<String> {
        self.target_rpc_contract_object_key.clone().or_else(|| {
            self.runtime_state_object_key
                .as_deref()
                .map(target_rpc_contract_object_key)
        })
    }

    pub fn to_browser_worker_input<'a>(
        &'a self,
        request_spec: &'a aistudio_web::AIStudioBrowserRequestSpec,
        timeout: Duration,
    ) -> Result<AIStudioBrowserWorkerInput<'a>, GatewayError> {
        Ok(AIStudioBrowserWorkerInput {
            runtime_state_object_key: self.require_runtime_state_object_key()?,
            app_url: self.app_url.as_str(),
            request_spec,
            timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
            result_file_path: Some(
                std::env::temp_dir()
                    .join(format!(
                        "aistudio-browser-worker-result-{}.json",
                        uuid::Uuid::new_v4()
                    ))
                    .to_string_lossy()
                    .to_string(),
            ),
            browser_executable_path: self.browser_executable_path.clone(),
        })
    }
}
