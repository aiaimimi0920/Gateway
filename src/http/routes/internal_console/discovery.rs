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
}

pub async fn discover_account(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<DiscoveryRequest>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let actor = state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
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
    let (base, key) = if let Some(id) = body.credential_id {
        let target = snapshot
            .select_credential_probe_target(&id)
            .ok_or_else(|| GatewayError::not_found("Credential not found."))?;
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
        )
    } else {
        (
            body.base_url.unwrap_or_default(),
            body.api_key.unwrap_or_default(),
        )
    };
    let discovery = crate::provider_discovery::discover(base.trim(), key.trim()).await
        .map_err(|_| GatewayError::bad_request("自动识别失败：未确认可用的模型目录和生成协议。请检查地址、密钥、认证方式及上游额度；超时不代表协议不支持，原配置未更改。")
            .with_code("provider_discovery_failed"))?;
    // Re-authenticate after the bounded network operation in case access was revoked.
    state
        .console_auth
        .verify_secret_grant(&request, &actor, grant)?;
    Ok(json_with_no_store(
        serde_json::json!({"discovery": discovery, "revision": snapshot.revision().id()}),
    ))
}
