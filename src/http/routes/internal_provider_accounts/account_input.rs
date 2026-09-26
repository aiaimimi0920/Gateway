//! Provider account write DTOs and lossless reconstruction of existing account inputs.

use crate::db;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAccountBody {
    pub label: String,
    #[serde(default)]
    pub service_provider_key: Option<String>,
    #[serde(default)]
    pub service_provider_label: Option<String>,
    pub adapter: String,
    pub protocol_family: String,
    #[serde(default)]
    pub protocol_profile: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub source_kind: Option<String>,
    #[serde(default)]
    pub aggregator_api_mode: Option<String>,
    #[serde(default)]
    pub web_reverse_access_mode: Option<String>,
    #[serde(default)]
    pub source_notes: Option<String>,
    #[serde(default)]
    pub execution_mode: Option<String>,
    #[serde(default)]
    pub endpoint_execution_modes: Option<std::collections::HashMap<String, String>>,
    pub payload: Value,
}

pub(super) fn into_input(body: ProviderAccountBody) -> db::UpsertProviderAccountInput {
    db::UpsertProviderAccountInput {
        label: body.label,
        service_provider_key: body.service_provider_key,
        service_provider_label: body.service_provider_label,
        adapter: body.adapter,
        protocol_family: body.protocol_family,
        protocol_profile: body.protocol_profile,
        status: body.status,
        source_kind: body.source_kind,
        aggregator_api_mode: body.aggregator_api_mode,
        web_reverse_access_mode: body.web_reverse_access_mode,
        source_notes: body.source_notes,
        execution_mode: body.execution_mode,
        endpoint_execution_modes: body.endpoint_execution_modes,
        payload: body.payload,
    }
}

pub(super) fn upsert_input_from_existing(
    existing: &db::GatewayProviderAccountView,
) -> db::UpsertProviderAccountInput {
    db::UpsertProviderAccountInput {
        label: existing.label.clone(),
        service_provider_key: Some(existing.service_provider_key.clone()),
        service_provider_label: Some(existing.service_provider_label.clone()),
        adapter: existing.adapter.clone(),
        protocol_family: existing.protocol_family.clone(),
        protocol_profile: Some(existing.protocol_profile.clone()),
        status: Some(existing.status.clone()),
        source_kind: existing.source_kind.clone(),
        aggregator_api_mode: existing.aggregator_api_mode.clone(),
        web_reverse_access_mode: existing.web_reverse_access_mode.clone(),
        source_notes: existing.source_notes.clone(),
        execution_mode: Some(match existing.execution_mode {
            crate::routing::candidate::ProviderExecutionMode::DirectHttp => {
                "direct_http".to_string()
            }
            crate::routing::candidate::ProviderExecutionMode::BrowserBacked => {
                "browser_backed".to_string()
            }
        }),
        endpoint_execution_modes: existing.endpoint_execution_modes.as_ref().map(|modes| {
            modes
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        match value {
                            crate::routing::candidate::ProviderExecutionMode::DirectHttp => {
                                "direct_http".to_string()
                            }
                            crate::routing::candidate::ProviderExecutionMode::BrowserBacked => {
                                "browser_backed".to_string()
                            }
                        },
                    )
                })
                .collect::<HashMap<_, _>>()
        }),
        payload: existing.payload.clone(),
    }
}
