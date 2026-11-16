//! 临时或已保存 SDK 账号的只读目录；配置 revision 同时作为账号代次栅栏。
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    console::copilot_catalog::{self as catalog, Settings},
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, DefaultBodyLimit, State},
    http::HeaderMap,
    response::Response,
    routing::post,
    Extension, Json, Router,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

const PATH: &str = "/v1/internal/gateway/console/copilot-model-catalog";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    github_token: Option<String>,
    provider_id: Option<String>,
    credential_id: Option<String>,
    expected_revision: String,
}

pub(crate) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    mount_with_settings(router, Settings::from_env())
}

fn mount_with_settings(
    router: Router<Arc<AppState>>,
    settings: Option<Settings>,
) -> Router<Arc<AppState>> {
    router.merge(
        Router::new()
            .route(PATH, post(read))
            .layer(Extension(settings))
            .layer(DefaultBodyLimit::max(32 * 1024))
            .layer(axum::middleware::map_response(super::dim_oauth_no_store)),
    )
}

async fn read(
    State(state): State<Arc<AppState>>,
    Extension(settings): Extension<Option<Settings>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(bearer): OptionalBearerToken,
    headers: HeaderMap,
    Json(input): Json<Input>,
) -> Result<Response, GatewayError> {
    authorize(&state, &headers, bearer.as_deref(), info.as_ref(), true)?;
    let snapshot = state.route_config.snapshot();
    let revision = snapshot.revision().id().to_string();
    if revision != input.expected_revision {
        return Err(GatewayError::conflict(
            "Configuration changed before catalog read",
        ));
    }
    let token = match (input.github_token, input.provider_id, input.credential_id) {
        (Some(token), None, None) => token,
        (None, Some(provider), Some(credential)) => snapshot
            .document()
            .providers
            .iter()
            .find(|p| p.id == provider && crate::routing::config::copilot::is_provider(p))
            .and_then(|p| {
                p.credentials
                    .iter()
                    .find(|c| c.id.as_ref() == Some(&credential))
            })
            .and_then(|c| c.copilot_github_token.clone())
            .ok_or_else(|| GatewayError::not_found("Copilot credential is unavailable"))?,
        _ => {
            return Err(GatewayError::bad_request(
                "Choose exactly one Copilot identity source",
            ))
        }
    };
    if !crate::routing::config::copilot::valid_token(&token) {
        return Err(GatewayError::bad_request("Invalid Copilot token"));
    }
    if state.shutdown.is_requested() || state.lifecycle.is_draining() {
        return Err(catalog::unavailable());
    }
    let settings = settings.ok_or_else(catalog::unavailable)?;
    let slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    let (mut tx, rx) = tokio::sync::oneshot::channel();
    let shutdown = state.shutdown.clone();
    let fence = catalog::RevisionFence {
        store: state.route_config.clone(),
        revision: revision.clone(),
    };
    if !state.lifecycle.background_tasks.spawn(
        crate::runtime::background_tasks::Worker::CopilotCatalog,
        &state.shutdown,
        async move {
            let _slot = slot;
            let result = catalog::run(settings, token, shutdown, &mut tx, fence).await;
            let _ = tx.send(result);
        },
    ) {
        return Err(GatewayError::conflict(
            "Copilot catalog is busy or stopping",
        ));
    }
    let models = rx.await.map_err(|_| catalog::unavailable())??;
    authorize(&state, &headers, bearer.as_deref(), info.as_ref(), true)?;
    if state.shutdown.is_requested()
        || state.lifecycle.is_draining()
        || state.route_config.snapshot().revision().id() != revision
    {
        return Err(GatewayError::conflict(
            "Configuration changed during catalog read",
        ));
    }
    Ok(json_with_no_store(
        serde_json::json!({"revision":revision,"catalog":{"models":models}}),
    ))
}

#[cfg(test)]
mod tests;
