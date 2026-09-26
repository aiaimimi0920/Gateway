use rquest::Client;
#[cfg(test)]
use rquest::Method;
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;

use crate::error::{classify_upstream_error, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::protocol::openai;
use crate::protocol::upstream_body::collect_bounded_upstream_text_with_provider;
use crate::routing::candidate::ProviderAccountPayload;
#[cfg(test)]
use crate::upstream::common::RequestPlan;

mod config;
mod request;
mod response_body;
mod run;
mod run_lifetime;
mod run_slots;
pub use run::RunRuntime;
#[cfg(test)]
use run::{acquire_run_lease, run_bucket, run_buckets, ManagedRun};
use run::{invalidate_run_lease, release_run_lease, FreeBuffRunLease};
mod session;
mod session_response;
mod transport;
pub use config::read_payload_string;
#[cfg(test)]
use config::{credential_subject_id, extract_agent_id};
use config::{read_payload_object, FreeBuffRuntimeConfig};
use request::build_chat_request_body;
pub use request::{build_request_plan, is_reserved_payload_extra_key, normalize_base_url};
#[cfg(test)]
use request::{flatten_freebuff_content_parts, sanitize_freebuff_messages};
use session::{ensure_free_session, record_waiting_room_rejection};
#[cfg(test)]
use session_response::{
    build_synthetic_session_snapshot, parse_free_session_snapshot, FreeBuffSessionState,
    FreeBuffWaitingRoomRejection,
};
use session_response::{
    classify_auth_error, classify_waiting_room_error, classify_waiting_room_rejection,
    should_retry_with_fresh_run,
};
use transport::send_chat_request;

#[cfg(test)]
mod config_tests;

#[cfg(test)]
mod request_tests;

#[cfg(test)]
mod session_response_tests;

#[cfg(test)]
mod deadline_tests;

#[cfg(test)]
mod run_lease_tests;

pub const FREEBUFF_DEFAULT_CHAT_COMPLETIONS_PATH: &str = "/api/v1/chat/completions";
pub const FREEBUFF_DEFAULT_AGENT_RUNS_PATH: &str = "/api/v1/agent-runs";
pub const FREEBUFF_DEFAULT_SESSION_PATH: &str = "/api/v1/freebuff/session";
pub const FREEBUFF_DEFAULT_USER_AGENT: &str = "ai-sdk/openai-compatible/1.0.25/codebuff";
pub const FREEBUFF_DEFAULT_COST_MODE: &str = "free";
pub const FREEBUFF_DEFAULT_ROTATION_SECS: u64 = 6 * 60 * 60;
pub const FREEBUFF_DEFAULT_SESSION_POLL_INTERVAL_MS: u64 = 1_000;
pub const FREEBUFF_DEFAULT_SESSION_POLL_TIMEOUT_MS: u64 = 15_000;
pub const FREEBUFF_INSTANCE_HEADER_NAME: &str = "x-freebuff-instance-id";

pub struct FreeBuffLeaseHandle {
    config: FreeBuffRuntimeConfig,
    lease: Option<FreeBuffRunLease>,
}

impl FreeBuffLeaseHandle {
    pub async fn release(mut self) {
        if let Some(lease) = self.lease.take() {
            release_run_lease(lease).await;
        }
    }

    pub async fn invalidate(mut self, reason: &str) {
        if let Some(lease) = self.lease.take() {
            invalidate_run_lease(&self.config, lease, reason).await;
        }
    }
}

pub struct FreeBuffStreamingExecution {
    pub response: rquest::Response,
    pub lease_handle: FreeBuffLeaseHandle,
}

pub async fn execute(
    runtime: &Arc<RunRuntime>,
    client: &Client,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let execution =
        execute_attempt(runtime, client, payload, req, model, extra_headers, false).await?;
    let provider = "freebuff_compatible";
    let FreeBuffStreamingExecution {
        response,
        lease_handle,
    } = execution;

    let body_result = response_body::read_chat_response(response).await;

    lease_handle.release().await;

    let body = body_result?;
    openai::unpack_openai_response(&body)
        .or_else(|_| responses_compatible_unpack(&body))
        .map_err(|error| error.with_provider(provider))
}

pub async fn execute_stream(
    runtime: &Arc<RunRuntime>,
    client: &Client,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<FreeBuffStreamingExecution, GatewayError> {
    execute_attempt(runtime, client, payload, req, model, extra_headers, true).await
}

pub async fn probe_payload(
    runtime: &Arc<RunRuntime>,
    client: &Client,
    payload: &ProviderAccountPayload,
) -> Result<(), GatewayError> {
    runtime.ensure_open()?;
    let model = payload
        .default_model
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            read_payload_object(payload.extra_body.as_ref(), &["freebuffModelAgentMap"])
                .and_then(|map| map.keys().next().map(String::as_str))
        })
        .unwrap_or("freebuff-probe-model");
    let config = FreeBuffRuntimeConfig::from_payload(payload, model)?;
    ensure_free_session(client, &config).await?;
    runtime.probe_run(client, &config).await
}

async fn execute_attempt(
    runtime: &Arc<RunRuntime>,
    client: &Client,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
    stream: bool,
) -> Result<FreeBuffStreamingExecution, GatewayError> {
    let provider = "freebuff_compatible";
    let config = FreeBuffRuntimeConfig::from_payload(payload, model)?;

    for attempt in 0..3 {
        runtime.ensure_open()?;
        let session_observation = ensure_free_session(client, &config).await?;
        let lease = runtime.acquire_run_lease(client, &config).await?;
        let body = build_chat_request_body(
            req,
            model,
            stream,
            &lease.run_id,
            &config.cost_mode,
            session_observation.instance_id(),
        );

        let response = match send_chat_request(
            client,
            &config.base_url,
            payload,
            &config.chat_completions_path,
            &body,
            extra_headers,
        )
        .await
        {
            Ok(response) => response,
            Err(error) => {
                if should_invalidate_run_after_error(&error) {
                    FreeBuffLeaseHandle {
                        config: config.clone(),
                        lease: Some(lease),
                    }
                    .invalidate(error.message.as_str())
                    .await;
                } else {
                    release_run_lease(lease).await;
                }
                return Err(error.with_provider(provider));
            }
        };

        let status = response.status().as_u16();
        if response.status().is_success() {
            return Ok(FreeBuffStreamingExecution {
                response,
                lease_handle: FreeBuffLeaseHandle {
                    config: config.clone(),
                    lease: Some(lease),
                },
            });
        }

        let body_text = match collect_bounded_upstream_text_with_provider(
            response,
            "FreeBuff chat error response",
            provider,
        )
        .await
        {
            Ok(body) => body,
            Err(error) => {
                release_run_lease(lease).await;
                return Err(error);
            }
        };
        if should_retry_with_fresh_run(status, &body_text) && attempt == 0 {
            FreeBuffLeaseHandle {
                config: config.clone(),
                lease: Some(lease),
            }
            .invalidate("freebuff upstream returned invalid run")
            .await;
            continue;
        }

        if let Some(rejection) = classify_waiting_room_rejection(status, &body_text) {
            release_run_lease(lease).await;
            record_waiting_room_rejection(&config, &session_observation, rejection, &body_text)
                .await;
            if attempt < 2 {
                continue;
            }
            return Err(
                classify_waiting_room_error(rejection, &body_text, &config).with_provider(provider)
            );
        }

        if matches!(status, 401 | 403) {
            FreeBuffLeaseHandle {
                config: config.clone(),
                lease: Some(lease),
            }
            .invalidate(body_text.as_str())
            .await;
            return Err(classify_auth_error(&body_text).with_provider(provider));
        }

        release_run_lease(lease).await;
        return Err(classify_upstream_error(status, &body_text, Some(provider)));
    }

    Err(
        GatewayError::server_error("FreeBuff request exhausted retry attempts without a valid run")
            .with_code("freebuff_run_retry_exhausted")
            .with_provider(provider),
    )
}

fn should_invalidate_run_after_error(error: &GatewayError) -> bool {
    matches!(
        error.kind,
        crate::error::ErrorKind::Timeout | crate::error::ErrorKind::Network
    )
}

fn normalize_lookup_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect()
}

fn responses_compatible_unpack(body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    crate::protocol::responses::unpack_responses_response(body)
}

#[cfg(test)]
mod tests;
