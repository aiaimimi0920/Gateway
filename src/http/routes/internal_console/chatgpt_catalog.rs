//! Catalog-only dry run, pinned to the official endpoint; callers commit with the revision fence.
use super::{console_request_context, json_with_no_store, required_console_management_token};
use crate::{
    error::GatewayError, http::extractors::OptionalBearerToken, protocol::chatgpt::codex_client,
    state::AppState, upstream::chatgpt::catalog as web_catalog,
};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRequest {
    credential_id: Option<String>,
    access_token: Option<String>,
    account_id: Option<String>,
}

fn validate_material(token: &str, account: &str) -> Result<(), GatewayError> {
    if token.is_empty()
        || token.len() > 16384
        || !token.bytes().all(|b| b.is_ascii_graphic())
        || account.is_empty()
        || account.len() > 256
        || !account.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(GatewayError::bad_request(
            "Invalid ChatGPT access token or Account ID",
        ));
    }
    Ok(())
}

pub async fn read_chatgpt_model_catalog(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CatalogRequest>,
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
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    let snapshot = state.route_config.snapshot();
    let catalog = if let Some(id) = body.credential_id {
        if body.access_token.is_some() || body.account_id.is_some() {
            return Err(GatewayError::bad_request(
                "Stored catalog requests cannot override token or Account ID",
            ));
        }
        let target = snapshot
            .select_credential_probe_target(&id)
            .ok_or_else(|| GatewayError::not_found("Credential not found"))?;
        let provider = snapshot
            .document()
            .providers
            .iter()
            .find(|p| {
                p.id == target.provider_id
                    && p.credentials.iter().any(|c| c.id.as_deref() == Some(&id))
            })
            .ok_or_else(|| {
                GatewayError::bad_request("Catalog requires an explicit ChatGPT account")
            })?;
        match provider.preset.as_deref() {
            Some("chatgpt-web-reverse") => {
                web_catalog::fetch(state.upstream_client.client(), &target.payload).await?
            }
            Some("chatgpt-codex-oauth-official-api") => {
                let account = target
                    .payload
                    .headers
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case("Chatgpt-Account-Id"))
                    .map(|(_, value)| value.as_str())
                    .unwrap_or_default();
                fetch_codex_catalog(
                    state.upstream_client.client(),
                    &target.payload.api_key,
                    account,
                )
                .await?
            }
            _ => {
                return Err(GatewayError::bad_request(
                    "Catalog requires an explicit ChatGPT Web or Codex account",
                ))
            }
        }
    } else {
        fetch_codex_catalog(
            state.upstream_client.client(),
            &body.access_token.unwrap_or_default(),
            &body.account_id.unwrap_or_default(),
        )
        .await?
    };
    state
        .console_auth
        .verify_secret_grant(&request, &actor, grant)?;
    if state.route_config.snapshot().revision().id() != snapshot.revision().id() {
        return Err(GatewayError::conflict(
            "Configuration changed during catalog read",
        ));
    }
    Ok(json_with_no_store(
        serde_json::json!({"revision":snapshot.revision().id(),"catalog":catalog}),
    ))
}

async fn fetch_codex_catalog(
    client: &rquest::Client,
    token: &str,
    account: &str,
) -> Result<serde_json::Value, GatewayError> {
    validate_material(token, account)?;
    let models = codex_client::catalog::fetch(client, token, account).await?;
    let checked_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    Ok(
        serde_json::json!({"models":models,"clientVersion":codex_client::VERSION,"checkedAt":checked_at}),
    )
}
