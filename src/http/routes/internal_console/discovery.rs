//! Secret-grant protected dry-run discovery. The caller commits using the normal revision fence.
use super::{console_request_context, json_with_no_store, required_console_management_token};
use crate::{error::GatewayError, http::extractors::OptionalBearerToken, state::AppState};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

use crate::provider_discovery::background::SLOTS as DISCOVERY_SLOTS;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryRequest {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub credential_id: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub catalog_only: bool,
}

pub async fn discover_account(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<DiscoveryRequest>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let management_token = required_console_management_token(token.as_deref(), &headers)?;
    let actor = state
        .console_auth
        .authenticate_management_token(&request, management_token)?;
    let grant = headers
        .get("x-secret-grant")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    state
        .console_auth
        .verify_secret_grant(&request, &actor, grant)?;
    let _slot = DISCOVERY_SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    let snapshot = state.route_config.snapshot();
    let (base, key, models) = if let Some(id) = body.credential_id {
        let target = snapshot
            .select_credential_probe_target(&id)
            .ok_or_else(|| GatewayError::not_found("Credential not found."))?;
        crate::protocol::lobsterai::management_boundary::reject_generic(&target.payload)?;
        if crate::protocol::commandcode::owns_payload(&target.payload) {
            return Err(crate::protocol::commandcode::invalid());
        }
        if crate::protocol::sap_aicore::owns_payload(&target.payload) {
            return Err(crate::protocol::sap_aicore::invalid());
        }
        if crate::protocol::vercel_free::owns_payload(&target.payload)
            || crate::protocol::kilo_free::owns_payload(&target.payload)
        {
            return Err(GatewayError::bad_request(
                "This provider uses a curated model catalog rather than generic account discovery.",
            ));
        }
        let provider = snapshot
            .document()
            .providers
            .iter()
            .find(|p| p.id == target.provider_id)
            .ok_or_else(|| GatewayError::not_found("Provider not found."))?;
        let cred = provider
            .credentials
            .iter()
            .find(|c| c.id.as_deref() == Some(&id))
            .ok_or_else(|| GatewayError::bad_request("Discovery requires an explicit account."))?;
        // Never send a stored key to a caller-supplied replacement address.
        if body.base_url.is_some() || body.api_key.is_some() {
            return Err(GatewayError::bad_request(
                "Stored-account discovery cannot override address or key.",
            ));
        }
        (
            cred.base_url
                .as_deref()
                .unwrap_or(&provider.base_url)
                .to_owned(),
            target.payload.api_key,
            if !body.models.is_empty() {
                body.models
            } else if !cred.supported_models.is_empty() {
                cred.supported_models.clone()
            } else {
                provider.supported_models.clone()
            },
        )
    } else {
        (
            body.base_url.unwrap_or_default(),
            body.api_key.unwrap_or_default(),
            body.models,
        )
    };
    let result = if body.catalog_only {
        crate::provider_discovery::read_account_models(base.trim(), key.trim())
            .await.map(|models| serde_json::json!({"catalog":{"models":models},"revision":snapshot.revision().id()}))
    } else {
        crate::provider_discovery::discover_with_models(base.trim(), key.trim(), &models)
            .await.map(|discovery| serde_json::json!({"discovery":discovery,"revision":snapshot.revision().id()}))
    };
    let result = result.map_err(|error| {
        GatewayError::bad_request(
            error
                .downcast_ref::<crate::provider_discovery::DiscoveryFailure>()
                .map(ToString::to_string)
                .unwrap_or_else(|| {
                    "自动识别失败：请检查地址、密钥及模型 ID 格式；原配置未更改。".into()
                }),
        )
        .with_code("provider_discovery_failed")
    })?;
    // A long dry-run may outlive its original grant. Re-authenticate the same token,
    // origin and client IP; only current (possibly renewed) authority may receive its result.
    let actor = state
        .console_auth
        .authenticate_management_token(&request, management_token)?;
    if !actor.secret_access_granted() {
        return Err(crate::console::auth::console_secret_access_required_error());
    }
    Ok(json_with_no_store(result))
}
