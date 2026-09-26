mod body_limit;
mod configuration;
mod listing;
mod local_paths;
mod object_io;
mod readiness;
#[cfg(test)]
mod tests;

pub(crate) use body_limit::STANDARD_OBJECT_LIMIT_BYTES;

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use aws_sdk_s3::Client;
use serde_json::Value;

use crate::error::GatewayError;
pub(crate) use local_paths::local_object_path;

const S3_NETWORK_DEADLINE: Duration = Duration::from_secs(30);

pub(super) fn network_timeout(operation: &str, deadline: Duration) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "{operation} object network deadline exceeded after {deadline:?}"
    ))
    .with_code("object_storage_network_timeout")
}

enum ObjectStorageDriver {
    Local {
        root: PathBuf,
    },
    S3Compatible {
        client: Client,
        bucket: String,
        deadline: Duration,
    },
}

pub struct GatewayObjectStorage {
    driver: ObjectStorageDriver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectStorageProbeOutcome {
    pub ready: bool,
    pub timed_out: bool,
}

pub fn gateway_object_storage() -> Result<&'static GatewayObjectStorage, GatewayError> {
    static STORAGE: OnceLock<Result<GatewayObjectStorage, String>> = OnceLock::new();
    match STORAGE.get_or_init(|| GatewayObjectStorage::from_env().map_err(|e| e.message)) {
        Ok(storage) => Ok(storage),
        Err(message) => Err(GatewayError::service_unavailable(message.clone())),
    }
}

pub fn build_gateway_provider_account_object_key(provider_account_id: &str) -> String {
    format!("ai-gateway/provider-account/{provider_account_id}.json")
}

pub fn build_gateway_provider_credential_object_key(provider_credential_id: &str) -> String {
    format!("ai-gateway/provider-credential/{provider_credential_id}.json")
}

pub fn build_gateway_analysis_export_prefix(export_id: &str) -> String {
    format!("ai-gateway/analysis-exports/{export_id}")
}

pub fn build_gateway_analysis_export_object_key(export_id: &str, file_name: &str) -> String {
    format!(
        "{}/{}",
        build_gateway_analysis_export_prefix(export_id),
        file_name
    )
}

pub fn build_gateway_analysis_export_manifest_object_key(export_id: &str) -> String {
    build_gateway_analysis_export_object_key(export_id, "manifest.json")
}

pub fn build_gateway_analysis_export_dataset_object_key(export_id: &str) -> String {
    build_gateway_analysis_export_object_key(export_id, "dataset.jsonl")
}

pub fn build_gateway_conversation_archive_object_key(archive_id: &str, file_name: &str) -> String {
    format!("ai-gateway/conversation-archives/{archive_id}/{file_name}")
}

pub fn build_gateway_conversation_archive_export_dataset_object_key(export_id: &str) -> String {
    format!("ai-gateway/conversation-archive-exports/{export_id}/dataset.jsonl")
}

pub fn build_gateway_conversation_dataset_export_object_key(
    dataset_id: &str,
    file_name: &str,
) -> String {
    format!("ai-gateway/conversation-dataset-exports/{dataset_id}/{file_name}")
}

pub fn choose_provider_payload_storage_mode(payload: &Value) -> &'static str {
    let serialized = serde_json::to_vec(payload).unwrap_or_default();
    if serialized.len() > 4096 {
        return "r2";
    }

    let has_large_array = payload
        .as_object()
        .map(|map| {
            map.values()
                .any(|value| matches!(value, Value::Array(values) if values.len() > 50))
        })
        .unwrap_or(false);
    if has_large_array {
        return "r2";
    }

    "inline"
}
