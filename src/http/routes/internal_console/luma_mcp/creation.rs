//! Explicit saved-account creation. Durable task admission precedes every billable operation.
mod execute;
mod identity;
mod review;
mod store;
mod workspace;
use super::super::chatgpt_oauth::authorize_actor;
use super::{no_store, resolve, revision_changed, Input};
use crate::{
    error::GatewayError, http::extractors::OptionalBearerToken,
    protocol::luma_mcp::creation as flow, state::AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    net::SocketAddr,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::{sync::Semaphore, time::Instant};

pub(super) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router.route(
        "/v1/internal/gateway/console/luma-mcp-creation",
        axum::routing::post(act)
            .layer(axum::extract::DefaultBodyLimit::max(32 * 1024))
            .layer(axum::middleware::map_response(no_store)),
    )
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Command {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
    action: Action,
    task_id: Option<String>,
    request: Option<flow::Request>,
    answer: Option<review::Answer>,
    #[serde(default)]
    charge_confirmed: bool,
}
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Action {
    Prepare,
    Start,
    Read,
    Cancel,
    End,
    Respond,
}
static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
async fn act(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(input): Json<Command>,
) -> Result<Response, GatewayError> {
    let persistent = state.local_runtime.is_none();
    let (actor, _) = authorize_actor(
        &state,
        &headers,
        token.as_deref(),
        info.as_ref(),
        true,
        persistent,
    )?;
    if state.shutdown.is_requested() || state.lifecycle.is_draining() {
        return Err(GatewayError::service_unavailable("Gateway is stopping."));
    }
    let journal = if let Some(local) = &state.local_runtime {
        store::Store::Local(local.clone())
    } else {
        let pool = state.pg_pool.clone().ok_or_else(|| {
            GatewayError::service_unavailable(
                "Luma MCP creation requires PostgreSQL in Server mode.",
            )
        })?;
        store::Store::server(
            pool,
            state.config.gateway_api_key_secret.as_deref(),
            actor.token_fingerprint(),
        )?
    };
    let target = Input {
        provider_id: input.provider_id,
        credential_id: input.credential_id,
        expected_revision: input.expected_revision,
    };
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != target.expected_revision {
        return Err(revision_changed());
    }
    let material = resolve(&snapshot, &target)?;
    if input.answer.is_some() != (input.action == Action::Respond) {
        return Err(invalid());
    }
    let mut task = if input.action == Action::Prepare {
        if input.task_id.is_some() {
            return Err(invalid());
        }
        let request = input.request.as_ref().ok_or_else(invalid)?;
        request.validate().map_err(|_| invalid())?;
        store::prepare(
            &journal,
            &target,
            request,
            state.route_config_runtime.as_deref(),
        )
        .await?
    } else {
        store::load(
            &journal,
            input.task_id.as_deref().ok_or_else(invalid)?,
            &target,
            state.route_config_runtime.as_deref(),
        )
        .await?
    };
    if input.action != Action::Prepare && journal.expired(&task).await? {
        if matches!(task.state.as_str(), "opening" | "submitting" | "responding") {
            review::interrupted(&mut task);
            task.state = "unconfirmed".into();
            task.error = Some("luma_mcp_operation_interrupted".into());
            task.lease_until = 0;
            journal.save(&mut task, true).await?;
        }
        if input.action == Action::Start {
            let request = input.request.as_ref().ok_or_else(invalid)?;
            request.validate().map_err(|_| invalid())?;
            if !input.charge_confirmed || task.request_hash != store::hash(request)? {
                return Err(invalid());
            }
            if task.state != "prepared" {
                return Err(GatewayError::conflict(
                    "Generation was already submitted; read the existing task.",
                ));
            }
            task.state = "opening".into();
        } else if input.action == Action::Respond {
            if input.request.is_some() {
                return Err(invalid());
            }
            review::admit(
                &mut task,
                input.answer.as_ref().ok_or_else(invalid)?,
                input.charge_confirmed,
            )?;
        } else if input.request.is_some() || input.charge_confirmed {
            return Err(invalid());
        }
        if input.action != Action::Read || task.session_id.is_some() {
            let permit = SLOTS
                .get_or_init(|| Arc::new(Semaphore::new(4)))
                .clone()
                .try_acquire_owned()
                .map_err(|_| GatewayError::conflict("Luma operations are busy."))?;
            // Start the local deadline before the database claim, not after a delayed commit.
            let deadline = Instant::now() + Duration::from_secs(80);
            journal.acquire(&mut task).await?;
            let endpoint = flow::endpoint();
            let drain = state
                .local_runtime
                .as_ref()
                .map(|local| local.track_finalizer());
            let owner = state.clone();
            let authorization = (
                headers.clone(),
                token.clone(),
                info,
                target.expected_revision.clone(),
            );
            // Only this bounded operation survives a disconnected HTTP waiter. It never retries generation.
            task = tokio::spawn(async move {
                let (_permit, _drain) = (permit, drain);
                let permitted = || {
                    Instant::now() < deadline
                        && authorize_actor(
                            &owner,
                            &authorization.0,
                            authorization.1.as_deref(),
                            authorization.2.as_ref(),
                            true,
                            persistent,
                        )
                        .is_ok()
                        && owner.route_config.snapshot().revision().id() == authorization.3
                        && !owner.lifecycle.is_draining()
                        && !owner.shutdown.is_requested()
                };
                execute::operation(
                    &journal,
                    task,
                    input.action,
                    input.request.as_ref(),
                    input.answer.as_ref(),
                    &material,
                    &endpoint,
                    &permitted,
                    deadline,
                )
                .await
            })
            .await
            .map_err(|_| {
                GatewayError::server_error("Luma operation interrupted; read the saved task.")
            })??;
        }
    }
    authorize_actor(
        &state,
        &headers,
        token.as_deref(),
        info.as_ref(),
        true,
        persistent,
    )?;
    if state.route_config.snapshot().revision().id() != target.expected_revision {
        return Err(revision_changed());
    }
    Ok(super::super::json_with_no_store(
        json!({"revision":target.expected_revision,
        "providerId":target.provider_id,"credentialId":target.credential_id,"task":task}),
    ))
}
fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid Luma creation request or task binding.")
        .with_code("luma_mcp_creation_invalid")
}
