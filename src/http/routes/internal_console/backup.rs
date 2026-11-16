//! One bounded, secret-grant protected entry for manual encrypted backups. No automatic cloud writes.
use super::{console_request_context, json_with_no_store, required_console_management_token};
use crate::{
    console::backup::{remote::Destination, service},
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, DefaultBodyLimit, FromRequest, Request, State},
    http::HeaderMap,
    response::Response,
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};
static OPERATION: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
enum Action {
    Status,
    SaveDestination {
        destination: Destination,
    },
    Export {
        password: String,
    },
    Preview {
        password: String,
        content: String,
    },
    Restore {
        password: String,
        content: String,
        #[serde(rename = "expectedRevision")]
        expected_revision: String,
        confirmed: bool,
    },
    Upload {
        password: String,
        #[serde(rename = "expectedRemoteRevision")]
        expected_remote_revision: Option<String>,
    },
    Download,
    CancelRestore,
}

pub(crate) fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/internal/gateway/console/backup", post(execute))
        .layer(DefaultBodyLimit::max(48 * 1024 * 1024))
        .layer(crate::http::middleware::body_limit_layer(48 * 1024 * 1024))
}

async fn execute(
    State(state): State<Arc<AppState>>,
    peer: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, GatewayError> {
    let context = console_request_context(&headers, peer.as_ref());
    let actor = state.console_auth.authenticate_management_token(
        &context,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    state.console_auth.verify_secret_grant(
        &context,
        &actor,
        headers
            .get("x-secret-grant")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default(),
    )?;
    let _permit = OPERATION
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Another backup operation is active"))?;
    let Json(action) = Json::<Action>::from_request(request, &state)
        .await
        .map_err(|_| GatewayError::bad_request("Invalid or oversized backup request"))?;
    let result = async {
        match action {
            Action::Status => {
                let root = service::root(&state)?;
                Ok(
                    serde_json::json!({"destination":service::destination(&state)?, "pending":gateway_local_data::restore::pending(&root).map_err(|e|e.to_string())?,
                    "revision":state.route_config.snapshot().revision().id()}),
                )
            }
            Action::SaveDestination { destination } => {
                service::save_destination(&state, destination)?;
                Ok(serde_json::json!({"saved":true}))
            }
            Action::Export { password } => Ok(
                serde_json::json!({"content":service::encode(service::export(&state, password).await?), "fileName":"gateway-backup.ngbk"}),
            ),
            Action::Preview { password, content } => {
                let preview =
                    service::preview(&state, service::decode_file(&content)?, password).await?;
                Ok(
                    serde_json::json!({"preview":preview,"revision":state.route_config.snapshot().revision().id()}),
                )
            }
            Action::Restore {
                password,
                content,
                expected_revision,
                confirmed,
            } => {
                if !confirmed {
                    return Err(
                        "Confirm replacement of accounts, access keys and balances first".into(),
                    );
                }
                service::restore(
                    &state,
                    service::decode_file(&content)?,
                    password,
                    &expected_revision,
                )
                .await?;
                Ok(serde_json::json!({"pending":true}))
            }
            Action::Upload {
                password,
                expected_remote_revision,
            } => {
                service::upload(&state, password, expected_remote_revision.as_deref()).await?;
                Ok(serde_json::json!({"uploaded":true}))
            }
            Action::Download => {
                let download = service::download(&state).await?;
                Ok(
                    serde_json::json!({"content":service::encode(download.bytes),"remoteRevision":download.revision,"fileName":"gateway-backup.ngbk"}),
                )
            }
            Action::CancelRestore => {
                gateway_local_data::restore::cancel(&service::root(&state)?)
                    .map_err(|error| error.to_string())?;
                Ok(serde_json::json!({"pending":false}))
            }
        }
    };
    let result: Result<serde_json::Value, String> = result.await;
    result
        .map(json_with_no_store)
        .map_err(|message| GatewayError::bad_request(message).with_code("console_backup_failed"))
}
