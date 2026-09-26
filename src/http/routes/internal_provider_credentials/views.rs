//! Credential response construction with quota and provider metadata.

use super::credential_payload::mask_provider_payload_secrets;
use super::display_metadata::{
    derive_credential_material_key, read_selected_display_model, read_supported_models,
    shared_provider_payload_hints,
};
use super::quota::load_provider_credential_quota;
use crate::db;
use crate::error::GatewayError;
use crate::provider_quota;
use crate::state::AppState;
use serde_json::Value;

pub(super) async fn build_provider_credential_response(
    state: &AppState,
    provider_account_payload: &Value,
    credential: db::GatewayProviderCredentialView,
    mask_secrets: bool,
) -> Result<Value, GatewayError> {
    let quota = match load_provider_credential_quota(state, credential.id.as_str(), false).await {
        Ok(value) => value,
        Err(error)
            if error
                .message
                .contains("deserialize provider payload for credential quota") =>
        {
            None
        }
        Err(error) => return Err(error),
    };
    Ok(build_provider_credential_response_value(
        provider_account_payload,
        credential,
        mask_secrets,
        quota,
    ))
}

pub(super) fn build_provider_credential_response_value(
    provider_account_payload: &Value,
    credential: db::GatewayProviderCredentialView,
    mask_secrets: bool,
    quota: Option<provider_quota::GatewayProviderQuotaView>,
) -> Value {
    let credential_payload = credential.payload.clone();
    serde_json::json!({
        "id": credential.id,
        "providerAccountId": credential.provider_account_id,
        "label": credential.label,
        "status": credential.status,
        "credential": if mask_secrets { mask_provider_payload_secrets(credential_payload.clone()) } else { credential_payload.clone() },
        "credentialMaterialKey": derive_credential_material_key(&credential_payload),
        "selectedDisplayModel": read_selected_display_model(&credential_payload),
        "supportedModels": read_supported_models(&credential_payload),
        "storageMode": credential.storage_mode,
        "sourceKind": credential.source_kind,
        "sourcePath": credential.source_path,
        "sourceHash": credential.source_hash,
        "syncMode": credential.sync_mode,
        "syncState": credential.sync_state,
        "syncError": credential.sync_error,
        "cooldownUntil": credential.cooldown_until,
        "lastError": credential.last_error,
        "failureCount": credential.failure_count,
        "lastHealthCheckAt": credential.last_health_check_at,
        "createdAt": credential.created_at,
        "updatedAt": credential.updated_at,
        "archivedAt": credential.archived_at,
        "providerQuota": quota,
        "sharedPayloadHints": shared_provider_payload_hints(provider_account_payload),
    })
}

pub(super) fn mask_provider_account_view(
    provider_account: db::GatewayProviderAccountView,
) -> Value {
    serde_json::json!({
        "id": provider_account.id,
        "label": provider_account.label,
        "adapter": provider_account.adapter,
        "protocolFamily": provider_account.protocol_family,
        "status": provider_account.status,
        "sourceProfile": {
            "sourceKind": provider_account.source_kind,
            "aggregatorApiMode": provider_account.aggregator_api_mode,
            "webReverseAccessMode": provider_account.web_reverse_access_mode,
            "notes": provider_account.source_notes,
            "derived": false,
        },
        "executionMode": provider_account.execution_mode,
        "endpointExecutionModes": provider_account.endpoint_execution_modes,
        "payload": mask_provider_payload_secrets(provider_account.payload),
        "createdAt": provider_account.created_at,
        "updatedAt": provider_account.updated_at,
        "cooldownUntil": provider_account.cooldown_until,
        "lastError": provider_account.last_error,
        "failureCount": provider_account.failure_count,
    })
}
