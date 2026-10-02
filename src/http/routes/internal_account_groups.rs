use std::net::SocketAddr;
use std::sync::Arc;

use crate::console::secrets::redact_url_value;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::routing::config::ActiveConfigSource;
use crate::state::AppState;
use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use axum::response::Response;

use super::internal_console::{
    console_request_context, json_with_no_store, required_console_management_token,
};

pub async fn get_account_groups_summary(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    let snapshot = state.route_config.snapshot();
    let mut inventory = snapshot.account_group_inventory();
    for account in &mut inventory.accounts {
        if let Some(base_url) = account.base_url.as_mut() {
            *base_url = redact_url_value(base_url);
        }
    }
    for provider in &mut inventory.providers {
        if let Some(base_url) = provider.base_url.as_mut() {
            *base_url = redact_url_value(base_url);
        }
    }

    Ok(json_with_no_store(serde_json::json!({
        "summary": {
            "routeConfigRevision": snapshot.revision().id(),
            "source": active_source_name(snapshot.source()),
            "accountGroups": inventory.account_groups,
            "accounts": inventory.accounts,
            "providers": inventory.providers,
        }
    })))
}

fn active_source_name(source: ActiveConfigSource) -> &'static str {
    match source {
        ActiveConfigSource::Yaml => "yaml",
        ActiveConfigSource::Redis => "redis",
        ActiveConfigSource::Database => "database",
        ActiveConfigSource::Recovered => "recovered",
    }
}

#[cfg(test)]
mod tests;
