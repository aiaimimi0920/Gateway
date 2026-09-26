use super::session_response::{
    build_synthetic_session_snapshot, build_waiting_room_queued_error, classify_auth_error,
    parse_free_session_snapshot, FreeBuffSessionSnapshot, FreeBuffSessionState,
    FreeBuffWaitingRoomRejection,
};
use super::transport::{build_absolute_url, build_runtime_headers};
use super::{FreeBuffRuntimeConfig, FREEBUFF_INSTANCE_HEADER_NAME};
use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::protocol::upstream_body::collect_bounded_upstream_text_with_provider;
#[cfg(test)]
use dashmap::DashMap;
use rquest::header::{HeaderMap, CONTENT_TYPE};
use rquest::{Client, Method};
use serde_json::Value;
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout_at};

#[cfg(test)]
#[path = "session_poll_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "body_tests.rs"]
mod body_tests;

#[path = "session_registry.rs"]
mod registry;
use registry::{FreeBuffSessionBucket, SessionRegistry};

static SESSION_BUCKETS: OnceLock<SessionRegistry> = OnceLock::new();

#[derive(Debug, Default, Clone)]
pub(super) struct SessionObservation {
    instance_id: Option<String>,
    source: Option<(Arc<Mutex<FreeBuffSessionBucket>>, Arc<()>)>,
}

impl SessionObservation {
    fn capture(
        bucket: &Arc<Mutex<FreeBuffSessionBucket>>,
        state: &FreeBuffSessionBucket,
        instance_id: Option<String>,
    ) -> Self {
        Self {
            instance_id,
            source: Some((bucket.clone(), state.revision.clone())),
        }
    }

    pub(super) fn instance_id(&self) -> Option<&str> {
        self.instance_id.as_deref()
    }
}

pub(super) async fn ensure_free_session(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
) -> Result<SessionObservation, GatewayError> {
    if !config.requires_free_session() {
        // Non-free requests never wait for session polling or fail because optional cache is full.
        return Ok(session_bucket(&config.session_bucket_key)
            .ok()
            .and_then(|bucket| {
                let state = bucket.try_lock().ok()?;
                Some(SessionObservation::capture(&bucket, &state, None))
            })
            .unwrap_or_default());
    }

    let deadline = Instant::now()
        .checked_add(config.session_poll_timeout)
        .ok_or_else(|| {
            GatewayError::bad_request("FreeBuff session poll timeout exceeds the clock range")
                .with_code("freebuff_invalid_session_poll_timeout")
                .with_provider("freebuff_compatible")
        })?;
    let timer_deadline = tokio::time::Instant::from_std(deadline);
    let bucket = session_bucket(config.session_bucket_key.as_str())?;
    // Queueing behind another poll consumes this request's budget too.
    let mut state = timeout_at(timer_deadline, bucket.lock())
        .await
        .map_err(|_| session_deadline_error())?;

    loop {
        if let Some(snapshot) = state.snapshot.as_ref() {
            match snapshot.state {
                FreeBuffSessionState::Active if snapshot.is_fresh() => {
                    let instance_id = snapshot.active_instance_id().ok_or_else(|| {
                        GatewayError::server_error(
                            "FreeBuff active waiting-room session missing instanceId",
                        )
                        .with_code("freebuff_missing_instance_id")
                        .with_provider("freebuff_compatible")
                    })?;
                    return Ok(SessionObservation::capture(
                        &bucket,
                        &state,
                        Some(instance_id.to_string()),
                    ));
                }
                FreeBuffSessionState::Disabled if snapshot.is_fresh() => {
                    return Ok(SessionObservation::capture(&bucket, &state, None));
                }
                FreeBuffSessionState::Queued if snapshot.is_fresh() => {
                    if Instant::now() >= deadline {
                        return Err(build_waiting_room_queued_error(snapshot, config));
                    }
                    let wait_for = snapshot
                        .refresh_after
                        .saturating_duration_since(Instant::now())
                        .min(config.session_poll_interval)
                        .min(deadline.saturating_duration_since(Instant::now()));
                    sleep(wait_for).await;
                    continue;
                }
                _ => {}
            }
        }

        let claimed_instance_id =
            state
                .snapshot
                .as_ref()
                .and_then(|snapshot| match snapshot.state {
                    FreeBuffSessionState::Queued | FreeBuffSessionState::Active => {
                        snapshot.instance_id.clone()
                    }
                    _ => None,
                });

        if Instant::now() >= deadline {
            return Err(session_deadline_error());
        }
        // The same deadline covers send and body decoding, not just the sleeps.
        let snapshot = timeout_at(timer_deadline, async {
            if claimed_instance_id.is_some() {
                get_free_session(client, config, claimed_instance_id.as_deref()).await
            } else {
                create_free_session(client, config).await
            }
        })
        .await
        .map_err(|_| session_deadline_error())??;
        state.set_snapshot(Some(snapshot.clone()));

        match snapshot.state {
            FreeBuffSessionState::Active => {
                let instance_id = snapshot.instance_id.clone().ok_or_else(|| {
                    GatewayError::server_error(
                        "FreeBuff active waiting-room session missing instanceId",
                    )
                    .with_code("freebuff_missing_instance_id")
                    .with_provider("freebuff_compatible")
                })?;
                return Ok(SessionObservation::capture(
                    &bucket,
                    &state,
                    Some(instance_id),
                ));
            }
            FreeBuffSessionState::Disabled => {
                return Ok(SessionObservation::capture(&bucket, &state, None))
            }
            FreeBuffSessionState::Queued => {
                if Instant::now() >= deadline {
                    return Err(build_waiting_room_queued_error(&snapshot, config));
                }
                sleep(
                    config
                        .session_poll_interval
                        .min(deadline.saturating_duration_since(Instant::now())),
                )
                .await;
            }
            FreeBuffSessionState::Draining
            | FreeBuffSessionState::Superseded
            | FreeBuffSessionState::Expired
            | FreeBuffSessionState::None => {
                if Instant::now() >= deadline {
                    return Err(session_deadline_error());
                }
                state.set_snapshot(None);
            }
        }
    }
}

fn session_deadline_error() -> GatewayError {
    GatewayError::service_unavailable(
        "FreeBuff waiting-room session could not become active in time",
    )
    .with_code("freebuff_session_unavailable")
    .with_provider("freebuff_compatible")
}

async fn create_free_session(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
) -> Result<FreeBuffSessionSnapshot, GatewayError> {
    send_free_session_request(client, config, Method::POST, None).await
}

async fn get_free_session(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
    claimed_instance_id: Option<&str>,
) -> Result<FreeBuffSessionSnapshot, GatewayError> {
    send_free_session_request(client, config, Method::GET, claimed_instance_id).await
}

async fn send_free_session_request(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
    method: Method,
    claimed_instance_id: Option<&str>,
) -> Result<FreeBuffSessionSnapshot, GatewayError> {
    let mut request = client
        .request(
            method,
            build_absolute_url(&config.base_url, &config.session_path),
        )
        .headers(build_session_headers(config));
    if let Some(instance_id) = claimed_instance_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        request = request.header(FREEBUFF_INSTANCE_HEADER_NAME, instance_id);
    }

    let response = request
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some("freebuff_compatible")))?;
    let status = response.status().as_u16();
    let body_text = collect_bounded_upstream_text_with_provider(
        response,
        "FreeBuff session response",
        "freebuff_compatible",
    )
    .await?;

    if matches!(status, 401 | 403) {
        return Err(classify_auth_error(&body_text));
    }
    if status < 200 || status >= 300 {
        return Err(classify_upstream_error(
            status,
            &body_text,
            Some("freebuff_compatible"),
        ));
    }

    let body: Value = serde_json::from_str(&body_text).map_err(|error| {
        GatewayError::server_error(format!("decode FreeBuff session response: {error}"))
            .with_code("freebuff_invalid_session_response")
            .with_provider("freebuff_compatible")
    })?;
    parse_free_session_snapshot(&body, config)
}

pub(super) async fn record_waiting_room_rejection(
    config: &FreeBuffRuntimeConfig,
    observation: &SessionObservation,
    rejection: FreeBuffWaitingRoomRejection,
    body: &str,
) {
    session_registry()
        .record_rejection(config, observation, rejection, body)
        .await;
}

fn build_session_headers(config: &FreeBuffRuntimeConfig) -> HeaderMap {
    let mut headers = build_runtime_headers(config);
    headers.remove(CONTENT_TYPE);
    headers
}

fn session_registry() -> &'static SessionRegistry {
    SESSION_BUCKETS.get_or_init(SessionRegistry::default)
}

#[cfg(test)]
fn session_buckets() -> &'static DashMap<String, Arc<Mutex<FreeBuffSessionBucket>>> {
    session_registry().buckets()
}

fn session_bucket(bucket_key: &str) -> Result<Arc<Mutex<FreeBuffSessionBucket>>, GatewayError> {
    session_registry().get_or_insert(bucket_key)
}
