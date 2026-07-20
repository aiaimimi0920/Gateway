use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use rand::seq::SliceRandom;
use rquest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use rquest::{Client, Method};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::warn;

use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind};
use crate::protocol::openai;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;
use crate::upstream::headers::build_upstream_headers_with;

pub const FREEBUFF_DEFAULT_CHAT_COMPLETIONS_PATH: &str = "/api/v1/chat/completions";
pub const FREEBUFF_DEFAULT_AGENT_RUNS_PATH: &str = "/api/v1/agent-runs";
pub const FREEBUFF_DEFAULT_SESSION_PATH: &str = "/api/v1/freebuff/session";
pub const FREEBUFF_DEFAULT_USER_AGENT: &str = "ai-sdk/openai-compatible/1.0.25/codebuff";
pub const FREEBUFF_DEFAULT_COST_MODE: &str = "free";
pub const FREEBUFF_DEFAULT_ROTATION_SECS: u64 = 6 * 60 * 60;
pub const FREEBUFF_DEFAULT_SESSION_POLL_INTERVAL_MS: u64 = 1_000;
pub const FREEBUFF_DEFAULT_SESSION_POLL_TIMEOUT_MS: u64 = 15_000;
pub const FREEBUFF_INSTANCE_HEADER_NAME: &str = "x-freebuff-instance-id";

static RUN_BUCKETS: OnceLock<DashMap<String, Arc<Mutex<FreeBuffRunBucket>>>> = OnceLock::new();
static SESSION_BUCKETS: OnceLock<DashMap<String, Arc<Mutex<FreeBuffSessionBucket>>>> =
    OnceLock::new();

#[derive(Debug, Default)]
struct FreeBuffRunBucket {
    active: Option<ManagedRun>,
    parked: Vec<ManagedRun>,
}

#[derive(Debug, Default)]
struct FreeBuffSessionBucket {
    snapshot: Option<FreeBuffSessionSnapshot>,
}

#[derive(Debug, Clone)]
struct ManagedRun {
    run_id: String,
    agent_id: String,
    started_at: Instant,
    inflight: u64,
    request_count: u64,
    should_finish: bool,
}

impl ManagedRun {
    fn new(run_id: String, agent_id: String) -> Self {
        Self {
            run_id,
            agent_id,
            started_at: Instant::now(),
            inflight: 0,
            request_count: 0,
            should_finish: false,
        }
    }

    fn is_stale(&self, rotation_interval: Duration) -> bool {
        self.started_at.elapsed() >= rotation_interval
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FreeBuffSessionState {
    Disabled,
    None,
    Queued,
    Active,
    Draining,
    Superseded,
    Expired,
}

#[derive(Debug, Clone)]
struct FreeBuffSessionSnapshot {
    state: FreeBuffSessionState,
    instance_id: Option<String>,
    queue_position: Option<u64>,
    queue_depth: Option<u64>,
    estimated_wait_ms: Option<u64>,
    _admitted_at: Option<String>,
    _expires_at: Option<String>,
    _grace_ends_at: Option<String>,
    message: Option<String>,
    _observed_at: Instant,
    refresh_after: Instant,
}

impl FreeBuffSessionSnapshot {
    fn is_fresh(&self) -> bool {
        Instant::now() < self.refresh_after
    }

    fn active_instance_id(&self) -> Option<&str> {
        match self.state {
            FreeBuffSessionState::Active => self.instance_id.as_deref(),
            _ => None,
        }
    }

    fn retry_delay_ms(&self, fallback_ms: u64) -> u64 {
        self.estimated_wait_ms.unwrap_or(fallback_ms).max(250)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FreeBuffWaitingRoomRejection {
    MissingInstance,
    WaitingRoomRequired,
    WaitingRoomQueued,
    SessionSuperseded,
    SessionExpired,
}

#[derive(Debug, Clone)]
struct FreeBuffRuntimeConfig {
    bucket_key: String,
    session_bucket_key: String,
    base_url: String,
    auth_token: String,
    agent_id: String,
    chat_completions_path: String,
    start_run_path: String,
    finish_run_path: String,
    session_path: String,
    rotation_interval: Duration,
    session_poll_interval: Duration,
    session_poll_timeout: Duration,
    cost_mode: String,
    user_agent: String,
}

#[derive(Debug)]
struct FreeBuffRunLease {
    bucket_key: String,
    run_id: String,
}

pub struct FreeBuffLeaseHandle {
    client: Client,
    config: FreeBuffRuntimeConfig,
    lease: Option<FreeBuffRunLease>,
}

impl FreeBuffLeaseHandle {
    pub async fn release(mut self) {
        if let Some(lease) = self.lease.take() {
            release_run_lease(&self.client, &self.config, lease).await;
        }
    }

    pub async fn invalidate(mut self, reason: &str) {
        if let Some(lease) = self.lease.take() {
            invalidate_run_lease(&self.client, &self.config, lease, reason).await;
        }
    }
}

pub struct FreeBuffStreamingExecution {
    pub response: rquest::Response,
    pub lease_handle: FreeBuffLeaseHandle,
}

pub async fn execute(
    client: &Client,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let execution = execute_attempt(client, payload, req, model, extra_headers, false).await?;
    let provider = "freebuff_compatible";
    let FreeBuffStreamingExecution {
        response,
        lease_handle,
    } = execution;

    let body_result: Result<Value, GatewayError> = response
        .json()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)));

    lease_handle.release().await;

    let body = body_result?;
    openai::unpack_openai_response(&body)
        .or_else(|_| responses_compatible_unpack(&body))
        .map_err(|error| error.with_provider(provider))
}

pub async fn execute_stream(
    client: &Client,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<FreeBuffStreamingExecution, GatewayError> {
    execute_attempt(client, payload, req, model, extra_headers, true).await
}

pub async fn probe_payload(
    client: &Client,
    payload: &ProviderAccountPayload,
) -> Result<(), GatewayError> {
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
    let run_id = start_run(client, &config).await?;
    let run = ManagedRun {
        run_id,
        agent_id: config.agent_id.clone(),
        started_at: Instant::now(),
        inflight: 0,
        request_count: 0,
        should_finish: true,
    };
    finish_run(client, &config, &run).await
}

pub fn normalize_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if let Some(rest) = trimmed.strip_prefix("https://codebuff.com") {
        return format!("https://www.codebuff.com{rest}");
    }
    if let Some(rest) = trimmed.strip_prefix("http://codebuff.com") {
        return format!("http://www.codebuff.com{rest}");
    }
    trimmed.to_string()
}

pub fn is_reserved_payload_extra_key(key: &str) -> bool {
    matches!(
        normalize_lookup_key(key).as_str(),
        "freebuffagentid"
            | "freebuffmodelagentmap"
            | "freebuffrunrotationsecs"
            | "freebuffagentrunspath"
            | "freebuffstartrunpath"
            | "freebufffinishrunpath"
            | "freebuffsessionpath"
            | "freebuffsessionpollintervalms"
            | "freebuffsessionpolltimeoutms"
            | "freebuffcostmode"
            | "freebuffuseragent"
    )
}

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    match req.endpoint_kind {
        EndpointKind::ChatCompletions
        | EndpointKind::Messages
        | EndpointKind::Responses
        | EndpointKind::Completions => {
            let path = payload
                .chat_completions_path
                .as_deref()
                .unwrap_or(FREEBUFF_DEFAULT_CHAT_COMPLETIONS_PATH);
            Ok(RequestPlan {
                method: Method::POST,
                url: format!("{}{}", normalize_base_url(&payload.base_url), path),
                query: Vec::new(),
                body: Some(openai::pack_openai(req, model, stream)),
                response_kind: EndpointKind::ChatCompletions,
            })
        }
        _ => Err(GatewayError::bad_request(
            "FreeBuff adapters currently support only chat/messages/responses style endpoints",
        )
        .with_code("unsupported_freebuff_endpoint")),
    }
}

async fn execute_attempt(
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
        let freebuff_instance_id = ensure_free_session(client, &config).await?;
        let lease = acquire_run_lease(client, &config).await?;
        let body = build_chat_request_body(
            req,
            model,
            stream,
            &lease.run_id,
            &config.cost_mode,
            freebuff_instance_id.as_deref(),
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
                        client: client.clone(),
                        config: config.clone(),
                        lease: Some(lease),
                    }
                    .invalidate(error.message.as_str())
                    .await;
                } else {
                    release_run_lease(client, &config, lease).await;
                }
                return Err(error.with_provider(provider));
            }
        };

        let status = response.status().as_u16();
        if response.status().is_success() {
            return Ok(FreeBuffStreamingExecution {
                response,
                lease_handle: FreeBuffLeaseHandle {
                    client: client.clone(),
                    config: config.clone(),
                    lease: Some(lease),
                },
            });
        }

        let body_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("<unreadable body>"));
        if should_retry_with_fresh_run(status, &body_text) && attempt == 0 {
            FreeBuffLeaseHandle {
                client: client.clone(),
                config: config.clone(),
                lease: Some(lease),
            }
            .invalidate("freebuff upstream returned invalid run")
            .await;
            continue;
        }

        if let Some(rejection) = classify_waiting_room_rejection(status, &body_text) {
            release_run_lease(client, &config, lease).await;
            record_waiting_room_rejection(
                &config,
                freebuff_instance_id.as_deref(),
                rejection,
                &body_text,
            )
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
                client: client.clone(),
                config: config.clone(),
                lease: Some(lease),
            }
            .invalidate(body_text.as_str())
            .await;
            return Err(classify_auth_error(&body_text).with_provider(provider));
        }

        release_run_lease(client, &config, lease).await;
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

impl FreeBuffRuntimeConfig {
    fn from_payload(payload: &ProviderAccountPayload, model: &str) -> Result<Self, GatewayError> {
        if payload.api_key.trim().is_empty() {
            return Err(GatewayError::unauthorized(
                "FreeBuff provider requires a non-empty auth token",
            )
            .with_code("freebuff_missing_auth_token")
            .with_provider("freebuff_compatible"));
        }

        let base_url = normalize_base_url(&payload.base_url);
        if base_url.is_empty() {
            return Err(
                GatewayError::bad_request("FreeBuff provider requires payload.baseUrl")
                    .with_code("freebuff_missing_base_url")
                    .with_provider("freebuff_compatible"),
            );
        }

        let extra = payload.extra_body.as_ref();
        let agent_runs_path = read_payload_string(extra, &["freebuffAgentRunsPath"])
            .unwrap_or_else(|| FREEBUFF_DEFAULT_AGENT_RUNS_PATH.to_string());
        let start_run_path = read_payload_string(extra, &["freebuffStartRunPath"])
            .unwrap_or_else(|| agent_runs_path.clone());
        let finish_run_path = read_payload_string(extra, &["freebuffFinishRunPath"])
            .unwrap_or_else(|| agent_runs_path.clone());
        let session_path = read_payload_string(extra, &["freebuffSessionPath"])
            .unwrap_or_else(|| FREEBUFF_DEFAULT_SESSION_PATH.to_string());
        let chat_completions_path = payload
            .chat_completions_path
            .clone()
            .unwrap_or_else(|| FREEBUFF_DEFAULT_CHAT_COMPLETIONS_PATH.to_string());
        let rotation_secs = read_payload_u64(extra, &["freebuffRunRotationSecs"])
            .unwrap_or(FREEBUFF_DEFAULT_ROTATION_SECS)
            .max(60);
        let session_poll_interval_ms = read_payload_u64(extra, &["freebuffSessionPollIntervalMs"])
            .unwrap_or(FREEBUFF_DEFAULT_SESSION_POLL_INTERVAL_MS)
            .max(250);
        let session_poll_timeout_ms = read_payload_u64(extra, &["freebuffSessionPollTimeoutMs"])
            .unwrap_or(FREEBUFF_DEFAULT_SESSION_POLL_TIMEOUT_MS)
            .max(session_poll_interval_ms);
        let cost_mode = read_payload_string(extra, &["freebuffCostMode"])
            .unwrap_or_else(|| FREEBUFF_DEFAULT_COST_MODE.to_string());
        let user_agent = read_payload_string(extra, &["freebuffUserAgent"])
            .unwrap_or_else(|| FREEBUFF_DEFAULT_USER_AGENT.to_string());
        let agent_id = resolve_agent_id(extra, model)?;
        let credential_subject = credential_subject_id(payload);

        Ok(Self {
            bucket_key: format!("{credential_subject}::{agent_id}"),
            session_bucket_key: credential_subject,
            base_url,
            auth_token: payload.api_key.trim().to_string(),
            agent_id,
            chat_completions_path,
            start_run_path,
            finish_run_path,
            session_path,
            rotation_interval: Duration::from_secs(rotation_secs),
            session_poll_interval: Duration::from_millis(session_poll_interval_ms),
            session_poll_timeout: Duration::from_millis(session_poll_timeout_ms),
            cost_mode,
            user_agent,
        })
    }

    fn requires_free_session(&self) -> bool {
        self.cost_mode.trim().eq_ignore_ascii_case("free")
    }
}

async fn acquire_run_lease(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
) -> Result<FreeBuffRunLease, GatewayError> {
    let bucket = run_bucket(config.bucket_key.as_str());
    let mut parked_to_finish = Vec::new();
    let mut state = bucket.lock().await;
    let needs_new_run = state
        .active
        .as_ref()
        .map(|run| run.is_stale(config.rotation_interval))
        .unwrap_or(true);

    if needs_new_run {
        let fresh_run_id = start_run(client, config).await?;
        let fresh_run = ManagedRun::new(fresh_run_id, config.agent_id.clone());
        if let Some(previous_active) = state.active.replace(fresh_run) {
            if previous_active.inflight == 0 {
                parked_to_finish.push(previous_active);
            } else {
                let mut draining = previous_active;
                draining.should_finish = true;
                state.parked.push(draining);
            }
        }
    }

    let active = state.active.as_mut().ok_or_else(|| {
        GatewayError::server_error("FreeBuff run acquisition ended without an active run")
            .with_code("freebuff_missing_active_run")
    })?;
    active.inflight += 1;
    active.request_count += 1;
    let lease = FreeBuffRunLease {
        bucket_key: config.bucket_key.clone(),
        run_id: active.run_id.clone(),
    };
    drop(state);

    for run in parked_to_finish {
        spawn_finish_run(client.clone(), config.clone(), run);
    }

    Ok(lease)
}

async fn release_run_lease(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
    lease: FreeBuffRunLease,
) {
    let Some(bucket) = run_buckets()
        .get(lease.bucket_key.as_str())
        .map(|entry| entry.value().clone())
    else {
        return;
    };

    let mut to_finish = Vec::new();
    let mut state = bucket.lock().await;
    if let Some(active) = state.active.as_mut() {
        if active.run_id == lease.run_id {
            active.inflight = active.inflight.saturating_sub(1);
        }
    }

    let mut parked_index = None;
    for (index, run) in state.parked.iter_mut().enumerate() {
        if run.run_id == lease.run_id {
            run.inflight = run.inflight.saturating_sub(1);
            if run.inflight == 0 {
                parked_index = Some(index);
            }
            break;
        }
    }

    if let Some(index) = parked_index {
        let run = state.parked.remove(index);
        if run.should_finish {
            to_finish.push(run);
        }
    }

    let remove_bucket = state.active.is_none() && state.parked.is_empty();
    drop(state);

    if remove_bucket {
        run_buckets().remove(lease.bucket_key.as_str());
    }

    for run in to_finish {
        spawn_finish_run(client.clone(), config.clone(), run);
    }
}

async fn invalidate_run_lease(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
    lease: FreeBuffRunLease,
    reason: &str,
) {
    let Some(bucket) = run_buckets()
        .get(lease.bucket_key.as_str())
        .map(|entry| entry.value().clone())
    else {
        return;
    };

    let mut state = bucket.lock().await;
    if state
        .active
        .as_ref()
        .is_some_and(|run| run.run_id == lease.run_id)
    {
        if let Some(mut invalidated) = state.active.take() {
            invalidated.inflight = invalidated.inflight.saturating_sub(1);
            invalidated.should_finish = false;
            if invalidated.inflight > 0 {
                state.parked.push(invalidated);
            }
        }
    } else if let Some(index) = state
        .parked
        .iter()
        .position(|run| run.run_id == lease.run_id)
    {
        let mut run = state.parked.remove(index);
        run.inflight = run.inflight.saturating_sub(1);
        run.should_finish = false;
        if run.inflight > 0 {
            state.parked.push(run);
        }
    }

    let remove_bucket = state.active.is_none() && state.parked.is_empty();
    drop(state);

    if remove_bucket {
        run_buckets().remove(lease.bucket_key.as_str());
    }

    if !reason.trim().is_empty() {
        warn!(
            bucket_key = %config.bucket_key,
            run_id = %lease.run_id,
            reason = reason.trim(),
            "freebuff run invalidated"
        );
    }

    let _ = client;
}

fn spawn_finish_run(client: Client, config: FreeBuffRuntimeConfig, run: ManagedRun) {
    tokio::spawn(async move {
        if let Err(error) = finish_run(&client, &config, &run).await {
            warn!(
                bucket_key = %config.bucket_key,
                agent_id = %run.agent_id,
                run_id = %run.run_id,
                error = %error,
                "freebuff finish run failed"
            );
        }
    });
}

async fn ensure_free_session(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
) -> Result<Option<String>, GatewayError> {
    if !config.requires_free_session() {
        return Ok(None);
    }

    let bucket = session_bucket(config.session_bucket_key.as_str());
    let mut state = bucket.lock().await;
    let deadline = Instant::now() + config.session_poll_timeout;

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
                    return Ok(Some(instance_id.to_string()));
                }
                FreeBuffSessionState::Disabled if snapshot.is_fresh() => {
                    return Ok(None);
                }
                FreeBuffSessionState::Queued if snapshot.is_fresh() => {
                    if Instant::now() >= deadline {
                        return Err(build_waiting_room_queued_error(snapshot, config));
                    }
                    let wait_for = snapshot
                        .refresh_after
                        .saturating_duration_since(Instant::now())
                        .min(config.session_poll_interval);
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

        let snapshot = if claimed_instance_id.is_some() {
            get_free_session(client, config, claimed_instance_id.as_deref()).await?
        } else {
            create_free_session(client, config).await?
        };
        state.snapshot = Some(snapshot.clone());

        match snapshot.state {
            FreeBuffSessionState::Active => {
                let instance_id = snapshot.instance_id.clone().ok_or_else(|| {
                    GatewayError::server_error(
                        "FreeBuff active waiting-room session missing instanceId",
                    )
                    .with_code("freebuff_missing_instance_id")
                    .with_provider("freebuff_compatible")
                })?;
                return Ok(Some(instance_id));
            }
            FreeBuffSessionState::Disabled => return Ok(None),
            FreeBuffSessionState::Queued => {
                if Instant::now() >= deadline {
                    return Err(build_waiting_room_queued_error(&snapshot, config));
                }
                sleep(config.session_poll_interval).await;
            }
            FreeBuffSessionState::Draining
            | FreeBuffSessionState::Superseded
            | FreeBuffSessionState::Expired
            | FreeBuffSessionState::None => {
                if Instant::now() >= deadline {
                    return Err(GatewayError::service_unavailable(
                        "FreeBuff waiting-room session could not become active in time",
                    )
                    .with_code("freebuff_session_unavailable")
                    .with_provider("freebuff_compatible"));
                }
                state.snapshot = None;
            }
        }
    }
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
    let body_text = response
        .text()
        .await
        .unwrap_or_else(|_| String::from("<unreadable body>"));

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

async fn record_waiting_room_rejection(
    config: &FreeBuffRuntimeConfig,
    instance_id: Option<&str>,
    rejection: FreeBuffWaitingRoomRejection,
    body: &str,
) {
    let bucket = session_bucket(config.session_bucket_key.as_str());
    let mut state = bucket.lock().await;
    state.snapshot = Some(build_synthetic_session_snapshot(
        rejection,
        instance_id,
        body,
        config,
    ));
}

fn build_session_headers(config: &FreeBuffRuntimeConfig) -> HeaderMap {
    let mut headers = build_runtime_headers(config);
    headers.remove(CONTENT_TYPE);
    headers
}

async fn start_run(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
) -> Result<String, GatewayError> {
    let response = send_run_action(
        client,
        config,
        &config.start_run_path,
        json!({
            "action": "START",
            "agentId": config.agent_id,
        }),
    )
    .await?;
    let status = response.status().as_u16();
    let body_text = response
        .text()
        .await
        .unwrap_or_else(|_| String::from("<unreadable body>"));
    if status < 200 || status >= 300 {
        if matches!(status, 401 | 403) {
            return Err(classify_auth_error(&body_text));
        }
        return Err(classify_upstream_error(
            status,
            &body_text,
            Some("freebuff_compatible"),
        ));
    }

    let body: Value = serde_json::from_str(&body_text).map_err(|error| {
        GatewayError::server_error(format!("decode FreeBuff START run response: {error}"))
            .with_code("freebuff_invalid_start_run_response")
            .with_provider("freebuff_compatible")
    })?;
    let run_id = body
        .get("runId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error("FreeBuff START run response missing runId")
                .with_code("freebuff_missing_run_id")
                .with_provider("freebuff_compatible")
        })?;
    Ok(run_id.to_string())
}

async fn finish_run(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
    run: &ManagedRun,
) -> Result<(), GatewayError> {
    let response = send_run_action(
        client,
        config,
        &config.finish_run_path,
        json!({
            "action": "FINISH",
            "runId": run.run_id,
            "status": "completed",
            "totalSteps": run.request_count,
            "directCredits": 0,
            "totalCredits": 0,
        }),
    )
    .await?;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        let body_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("<unreadable body>"));
        return Err(classify_upstream_error(
            status,
            &body_text,
            Some("freebuff_compatible"),
        ));
    }
    Ok(())
}

async fn send_run_action(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
    path: &str,
    body: Value,
) -> Result<rquest::Response, GatewayError> {
    let headers = build_runtime_headers(config);
    client
        .request(Method::POST, build_absolute_url(&config.base_url, path))
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some("freebuff_compatible")))
}

async fn send_chat_request(
    client: &Client,
    base_url: &str,
    payload: &ProviderAccountPayload,
    path: &str,
    body: &Value,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<rquest::Response, GatewayError> {
    let headers = build_upstream_headers_with(payload, extra_headers);
    client
        .request(Method::POST, build_absolute_url(base_url, path))
        .headers(headers)
        .json(body)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some("freebuff_compatible")))
}

fn build_runtime_headers(config: &FreeBuffRuntimeConfig) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/event-stream"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", config.auth_token)) {
        headers.insert(AUTHORIZATION, value);
    }
    if let Ok(value) = HeaderValue::from_str(config.user_agent.as_str()) {
        headers.insert(USER_AGENT, value);
    }
    headers
}

fn build_chat_request_body(
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
    run_id: &str,
    cost_mode: &str,
    freebuff_instance_id: Option<&str>,
) -> Value {
    let mut body = openai::pack_openai(req, model, stream);
    sanitize_freebuff_messages(&mut body);
    let object = body
        .as_object_mut()
        .expect("OpenAI-compatible payload must serialize as an object");
    let metadata = object
        .entry("codebuff_metadata".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(metadata) = metadata.as_object_mut() {
        metadata.insert("run_id".to_string(), Value::String(run_id.to_string()));
        metadata.insert(
            "cost_mode".to_string(),
            Value::String(cost_mode.trim().to_string()),
        );
        metadata.insert(
            "client_id".to_string(),
            Value::String(generate_client_session_id()),
        );
        if let Some(instance_id) = freebuff_instance_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            metadata.insert(
                "freebuff_instance_id".to_string(),
                Value::String(instance_id.to_string()),
            );
        }
    }
    body
}

fn sanitize_freebuff_messages(body: &mut Value) {
    let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) else {
        return;
    };

    for message in messages {
        let Some(content) = message.get_mut("content") else {
            continue;
        };

        let Some(parts) = content.as_array() else {
            continue;
        };

        if let Some(flattened) = flatten_freebuff_content_parts(parts) {
            *content = Value::String(flattened);
        }
    }
}

fn flatten_freebuff_content_parts(parts: &[Value]) -> Option<String> {
    let mut segments = Vec::with_capacity(parts.len());
    for part in parts {
        let object = part.as_object()?;
        match object.get("type").and_then(Value::as_str) {
            Some("text") => {
                let text = object.get("text").and_then(Value::as_str)?;
                segments.push(text.to_string());
            }
            Some("json") => {
                let value = object.get("value")?;
                segments.push(value.to_string());
            }
            Some("input_text") => {
                let text = object
                    .get("text")
                    .or_else(|| object.get("value"))
                    .and_then(Value::as_str)?;
                segments.push(text.to_string());
            }
            Some("output_text") => {
                let text = object.get("text").and_then(Value::as_str)?;
                segments.push(text.to_string());
            }
            Some("image_url") | Some("input_image") => return None,
            _ => {
                if let Some(text) = object.get("text").and_then(Value::as_str) {
                    segments.push(text.to_string());
                } else if let Some(value) = object.get("value") {
                    segments.push(value.to_string());
                } else {
                    return None;
                }
            }
        }
    }

    Some(segments.join("\n"))
}

fn resolve_agent_id(
    extra: Option<&HashMap<String, Value>>,
    model: &str,
) -> Result<String, GatewayError> {
    if let Some(agent_id) = resolve_model_agent(extra, model) {
        return Ok(agent_id);
    }
    if let Some(agent_id) = read_payload_string(extra, &["freebuffAgentId"]) {
        return Ok(agent_id);
    }
    Err(
        GatewayError::bad_request(format!(
            "FreeBuff provider 缺少 model `{model}` 对应的 agentId，请配置 extraBody.freebuffModelAgentMap 或 extraBody.freebuffAgentId"
        ))
        .with_code("freebuff_missing_agent_id")
        .with_provider("freebuff_compatible"),
    )
}

fn resolve_model_agent(extra: Option<&HashMap<String, Value>>, model: &str) -> Option<String> {
    let map = read_payload_object(extra, &["freebuffModelAgentMap"])?;
    let direct = map
        .get(model)
        .or_else(|| map.get(model.trim()))
        .or_else(|| map.get(&model.to_ascii_lowercase()))?;
    extract_agent_id(direct)
}

fn extract_agent_id(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Array(values) => {
            let candidates = values
                .iter()
                .filter_map(extract_agent_id)
                .collect::<Vec<_>>();
            let mut rng = rand::thread_rng();
            candidates.choose(&mut rng).cloned()
        }
        Value::Object(object) => object
            .get("agentId")
            .or_else(|| object.get("agent_id"))
            .or_else(|| object.get("id"))
            .and_then(extract_agent_id)
            .or_else(|| object.get("agents").and_then(extract_agent_id))
            .or_else(|| object.get("agentIds").and_then(extract_agent_id))
            .or_else(|| object.get("agent_ids").and_then(extract_agent_id)),
        _ => None,
    }
}

pub fn read_payload_string(
    extra: Option<&HashMap<String, Value>>,
    aliases: &[&str],
) -> Option<String> {
    let extra = extra?;
    let alias_keys = aliases
        .iter()
        .map(|key| normalize_lookup_key(key))
        .collect::<Vec<_>>();
    extra.iter().find_map(|(key, value)| {
        if !alias_keys.contains(&normalize_lookup_key(key)) {
            return None;
        }
        match value {
            Value::String(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(flag) => Some(flag.to_string()),
            _ => None,
        }
    })
}

fn read_payload_u64(extra: Option<&HashMap<String, Value>>, aliases: &[&str]) -> Option<u64> {
    let extra = extra?;
    let alias_keys = aliases
        .iter()
        .map(|key| normalize_lookup_key(key))
        .collect::<Vec<_>>();
    extra.iter().find_map(|(key, value)| {
        if !alias_keys.contains(&normalize_lookup_key(key)) {
            return None;
        }
        value
            .as_u64()
            .or_else(|| value.as_i64().map(|number| number.max(0) as u64))
            .or_else(|| {
                value
                    .as_str()
                    .and_then(|text| text.trim().parse::<u64>().ok())
            })
    })
}

fn read_payload_object<'a>(
    extra: Option<&'a HashMap<String, Value>>,
    aliases: &[&str],
) -> Option<&'a serde_json::Map<String, Value>> {
    let extra = extra?;
    let alias_keys = aliases
        .iter()
        .map(|key| normalize_lookup_key(key))
        .collect::<Vec<_>>();
    extra.iter().find_map(|(key, value)| {
        if !alias_keys.contains(&normalize_lookup_key(key)) {
            return None;
        }
        value.as_object()
    })
}

fn parse_free_session_snapshot(
    body: &Value,
    config: &FreeBuffRuntimeConfig,
) -> Result<FreeBuffSessionSnapshot, GatewayError> {
    let observed_at = Instant::now();
    let view = body
        .get("session")
        .or_else(|| body.get("data"))
        .unwrap_or(body);
    let status_text = read_json_string(view, &["status"])
        .unwrap_or_else(|| "none".to_string())
        .to_ascii_lowercase();
    let state = match status_text.as_str() {
        "disabled" => FreeBuffSessionState::Disabled,
        "none" => FreeBuffSessionState::None,
        "queued" => FreeBuffSessionState::Queued,
        "active" => FreeBuffSessionState::Active,
        "ended" => FreeBuffSessionState::Draining,
        "superseded" => FreeBuffSessionState::Superseded,
        other => {
            return Err(GatewayError::server_error(format!(
                "Unsupported FreeBuff session state `{other}`"
            ))
            .with_code("freebuff_unknown_session_state")
            .with_provider("freebuff_compatible"));
        }
    };

    let instance_id = read_json_string(view, &["instanceId", "claimedInstanceId"]);
    let queue_position = read_json_u64(view, &["position", "queuePosition"]);
    let queue_depth = read_json_u64(view, &["queueDepth"]);
    let estimated_wait_ms = read_json_u64(view, &["estimatedWaitMs", "estimatedWaitMillis"]);
    let remaining_ms = read_json_u64(view, &["remainingMs", "remainingMillis"]);
    let grace_remaining_ms =
        read_json_u64(view, &["gracePeriodRemainingMs", "graceRemainingMillis"]);
    let admitted_at = read_json_string(view, &["admittedAt"]);
    let expires_at = read_json_string(view, &["expiresAt"]);
    let grace_ends_at = read_json_string(view, &["gracePeriodEndsAt", "graceEndsAt"]);
    let message = read_json_string(view, &["message", "reason"])
        .or_else(|| read_json_string(body, &["message", "reason"]));

    if matches!(state, FreeBuffSessionState::Active) && instance_id.is_none() {
        return Err(GatewayError::server_error(
            "FreeBuff active session response missing instanceId",
        )
        .with_code("freebuff_missing_instance_id")
        .with_provider("freebuff_compatible"));
    }

    let refresh_delay = match state {
        FreeBuffSessionState::Active => Duration::from_millis(
            remaining_ms
                .unwrap_or(30_000)
                .saturating_sub(1_000)
                .max(1_000),
        ),
        FreeBuffSessionState::Queued => Duration::from_millis(
            estimated_wait_ms
                .unwrap_or(config.session_poll_interval.as_millis() as u64)
                .min(config.session_poll_interval.as_millis() as u64)
                .max(250),
        ),
        FreeBuffSessionState::Draining => Duration::from_millis(
            grace_remaining_ms
                .unwrap_or(0)
                .min(config.session_poll_interval.as_millis() as u64),
        ),
        FreeBuffSessionState::Disabled => Duration::from_secs(300),
        FreeBuffSessionState::None
        | FreeBuffSessionState::Superseded
        | FreeBuffSessionState::Expired => Duration::from_millis(0),
    };

    Ok(FreeBuffSessionSnapshot {
        state,
        instance_id,
        queue_position,
        queue_depth,
        estimated_wait_ms,
        _admitted_at: admitted_at,
        _expires_at: expires_at,
        _grace_ends_at: grace_ends_at,
        message,
        _observed_at: observed_at,
        refresh_after: observed_at + refresh_delay,
    })
}

fn build_synthetic_session_snapshot(
    rejection: FreeBuffWaitingRoomRejection,
    instance_id: Option<&str>,
    body: &str,
    config: &FreeBuffRuntimeConfig,
) -> FreeBuffSessionSnapshot {
    let observed_at = Instant::now();
    let state = match rejection {
        FreeBuffWaitingRoomRejection::MissingInstance
        | FreeBuffWaitingRoomRejection::WaitingRoomRequired => FreeBuffSessionState::None,
        FreeBuffWaitingRoomRejection::WaitingRoomQueued => FreeBuffSessionState::Queued,
        FreeBuffWaitingRoomRejection::SessionSuperseded => FreeBuffSessionState::Superseded,
        FreeBuffWaitingRoomRejection::SessionExpired => FreeBuffSessionState::Expired,
    };
    let refresh_delay = match state {
        FreeBuffSessionState::Queued => config.session_poll_interval,
        _ => Duration::from_millis(0),
    };

    FreeBuffSessionSnapshot {
        state,
        instance_id: instance_id.map(str::to_string),
        queue_position: None,
        queue_depth: None,
        estimated_wait_ms: Some(config.session_poll_interval.as_millis() as u64),
        _admitted_at: None,
        _expires_at: None,
        _grace_ends_at: None,
        message: extract_json_message(body),
        _observed_at: observed_at,
        refresh_after: observed_at + refresh_delay,
    }
}

fn build_waiting_room_queued_error(
    snapshot: &FreeBuffSessionSnapshot,
    config: &FreeBuffRuntimeConfig,
) -> GatewayError {
    let mut message = snapshot
        .message
        .clone()
        .unwrap_or_else(|| "FreeBuff waiting room is still queued".to_string());
    if let Some(position) = snapshot.queue_position {
        message.push_str(&format!("; position={position}"));
    }
    if let Some(depth) = snapshot.queue_depth {
        message.push_str(&format!("; queue_depth={depth}"));
    }
    if let Some(wait_ms) = snapshot.estimated_wait_ms {
        message.push_str(&format!("; estimated_wait_ms={wait_ms}"));
    }
    GatewayError::rate_limited(
        message,
        snapshot.retry_delay_ms(config.session_poll_interval.as_millis() as u64),
    )
    .with_code("waiting_room_queued")
    .with_provider("freebuff_compatible")
}

fn classify_waiting_room_rejection(
    status: u16,
    body: &str,
) -> Option<FreeBuffWaitingRoomRejection> {
    let lower = body.to_ascii_lowercase();
    let code = extract_json_code(body)
        .unwrap_or_default()
        .to_ascii_lowercase();

    match status {
        426 => Some(FreeBuffWaitingRoomRejection::MissingInstance),
        428 => Some(FreeBuffWaitingRoomRejection::WaitingRoomRequired),
        429 => {
            if code.contains("waiting_room_queued")
                || lower.contains("waiting_room_queued")
                || lower.contains("queue")
            {
                Some(FreeBuffWaitingRoomRejection::WaitingRoomQueued)
            } else {
                None
            }
        }
        409 => Some(FreeBuffWaitingRoomRejection::SessionSuperseded),
        410 => Some(FreeBuffWaitingRoomRejection::SessionExpired),
        _ => None,
    }
}

fn classify_waiting_room_error(
    rejection: FreeBuffWaitingRoomRejection,
    body: &str,
    config: &FreeBuffRuntimeConfig,
) -> GatewayError {
    match rejection {
        FreeBuffWaitingRoomRejection::MissingInstance => {
            GatewayError::bad_request(extract_json_message(body).unwrap_or_else(|| {
                "FreeBuff free-mode request is missing freebuff_instance_id".to_string()
            }))
            .with_code("freebuff_update_required")
        }
        FreeBuffWaitingRoomRejection::WaitingRoomRequired => GatewayError::service_unavailable(
            extract_json_message(body)
                .unwrap_or_else(|| "FreeBuff waiting-room session is required".to_string()),
        )
        .with_code("waiting_room_required"),
        FreeBuffWaitingRoomRejection::WaitingRoomQueued => GatewayError::rate_limited(
            extract_json_message(body)
                .unwrap_or_else(|| "FreeBuff waiting room is still queued".to_string()),
            config.session_poll_interval.as_millis() as u64,
        )
        .with_code("waiting_room_queued"),
        FreeBuffWaitingRoomRejection::SessionSuperseded => GatewayError::conflict(
            extract_json_message(body)
                .unwrap_or_else(|| "FreeBuff session instance was superseded".to_string()),
        )
        .with_code("session_superseded"),
        FreeBuffWaitingRoomRejection::SessionExpired => GatewayError::rate_limited(
            extract_json_message(body)
                .unwrap_or_else(|| "FreeBuff waiting-room session expired".to_string()),
            config.session_poll_interval.as_millis() as u64,
        )
        .with_code("session_expired"),
    }
}

fn read_json_string(value: &Value, aliases: &[&str]) -> Option<String> {
    let object = value.as_object()?;
    let alias_keys = aliases
        .iter()
        .map(|key| normalize_lookup_key(key))
        .collect::<Vec<_>>();
    object.iter().find_map(|(key, value)| {
        if !alias_keys.contains(&normalize_lookup_key(key)) {
            return None;
        }
        match value {
            Value::String(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(flag) => Some(flag.to_string()),
            _ => None,
        }
    })
}

fn read_json_u64(value: &Value, aliases: &[&str]) -> Option<u64> {
    let object = value.as_object()?;
    let alias_keys = aliases
        .iter()
        .map(|key| normalize_lookup_key(key))
        .collect::<Vec<_>>();
    object.iter().find_map(|(key, value)| {
        if !alias_keys.contains(&normalize_lookup_key(key)) {
            return None;
        }
        value
            .as_u64()
            .or_else(|| value.as_i64().map(|number| number.max(0) as u64))
            .or_else(|| {
                value
                    .as_str()
                    .and_then(|text| text.trim().parse::<u64>().ok())
            })
    })
}

fn extract_json_message(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    read_json_string(
        value.get("error").unwrap_or(&value),
        &["message", "detail", "reason"],
    )
    .or_else(|| read_json_string(&value, &["message", "detail", "reason"]))
}

fn extract_json_code(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    read_json_string(value.get("error").unwrap_or(&value), &["code", "errorCode"])
        .or_else(|| read_json_string(&value, &["code", "errorCode"]))
}

fn should_retry_with_fresh_run(status: u16, body: &str) -> bool {
    if status != 400 {
        return false;
    }
    let lower = body.to_ascii_lowercase();
    lower.contains("runid not found") || lower.contains("runid not running")
}

fn classify_auth_error(body: &str) -> GatewayError {
    let trimmed = body.trim();
    let lower = trimmed.to_ascii_lowercase();
    let message = if lower.contains("token_invalidated")
        || lower.contains("token revoked")
        || lower.contains("token_revoked")
        || lower.contains("invalid api key")
        || lower.contains("invalid_api_key")
    {
        if trimmed.is_empty() {
            "token_invalidated".to_string()
        } else {
            trimmed.to_string()
        }
    } else if trimmed.is_empty() {
        "token_invalidated: FreeBuff auth token unauthorized".to_string()
    } else {
        format!("token_invalidated: {trimmed}")
    };

    GatewayError::unauthorized(message)
        .with_code("token_invalidated")
        .with_provider("freebuff_compatible")
}

fn normalize_lookup_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect()
}

fn credential_subject_id(payload: &ProviderAccountPayload) -> String {
    if let Some(credential_id) = payload
        .credential_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!("cred:{credential_id}");
    }

    let mut hasher = Sha256::new();
    hasher.update(payload.base_url.trim().as_bytes());
    hasher.update(b":");
    hasher.update(payload.api_key.trim().as_bytes());
    let digest = hasher.finalize();
    format!("payload:{}", hex::encode(&digest[..12]))
}

fn build_absolute_url(base_url: &str, path: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    let normalized_path = if path.trim().starts_with('/') {
        path.trim().to_string()
    } else {
        format!("/{}", path.trim())
    };
    format!("{base}{normalized_path}")
}

fn generate_client_session_id() -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::thread_rng();
    (0..13)
        .map(|_| {
            let index = rand::Rng::gen_range(&mut rng, 0..ALPHABET.len());
            ALPHABET[index] as char
        })
        .collect()
}

fn responses_compatible_unpack(body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    crate::protocol::responses::unpack_responses_response(body)
}

fn run_buckets() -> &'static DashMap<String, Arc<Mutex<FreeBuffRunBucket>>> {
    RUN_BUCKETS.get_or_init(DashMap::new)
}

fn run_bucket(bucket_key: &str) -> Arc<Mutex<FreeBuffRunBucket>> {
    run_buckets()
        .entry(bucket_key.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(FreeBuffRunBucket::default())))
        .clone()
}

fn session_buckets() -> &'static DashMap<String, Arc<Mutex<FreeBuffSessionBucket>>> {
    SESSION_BUCKETS.get_or_init(DashMap::new)
}

fn session_bucket(bucket_key: &str) -> Arc<Mutex<FreeBuffSessionBucket>> {
    session_buckets()
        .entry(bucket_key.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(FreeBuffSessionBucket::default())))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };

    fn make_payload() -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: "freebuff_compatible".to_string(),
            base_url: "https://codebuff.com".to_string(),
            api_key: "fb-token".to_string(),
            credential_id: Some("cred-1".to_string()),
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some("z-ai/glm-5.1".to_string()),
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: Some(
                [
                    (
                        "freebuffModelAgentMap".to_string(),
                        json!({
                            "z-ai/glm-5.1": "base2-free",
                            "google/gemini-3.1-flash-lite-preview": ["basher", "researcher-web"]
                        }),
                    ),
                    ("freebuffRunRotationSecs".to_string(), json!(900)),
                ]
                .into_iter()
                .collect(),
            ),
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request() -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("z-ai/glm-5.1".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello".to_string(),
                }],
                name: None,
                tool_calls: Vec::new(),
                tool_call_id: None,
            }],
            tools: Vec::new(),
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn make_responses_request() -> CanonicalRelayRequest {
        let mut request = make_request();
        request.endpoint_kind = EndpointKind::Responses;
        request
    }

    fn assert_freebuff_chat_surface_plan(plan: &RequestPlan) {
        assert_eq!(plan.url, "https://www.codebuff.com/api/v1/chat/completions");
        assert_eq!(plan.response_kind, EndpointKind::ChatCompletions);
        assert_eq!(plan.body.as_ref().unwrap()["model"], "z-ai/glm-5.1");
        assert_eq!(plan.body.as_ref().unwrap()["stream"], true);
        assert!(plan.body.as_ref().unwrap().get("messages").is_some());
        assert!(plan.body.as_ref().unwrap().get("freebuffAgentId").is_none());
    }

    #[test]
    fn reserves_runtime_extra_body_keys() {
        assert!(is_reserved_payload_extra_key("freebuffAgentId"));
        assert!(is_reserved_payload_extra_key("freebuffModelAgentMap"));
        assert!(is_reserved_payload_extra_key("freebuffRunRotationSecs"));
        assert!(is_reserved_payload_extra_key("freebuffSessionPath"));
        assert!(is_reserved_payload_extra_key(
            "freebuffSessionPollIntervalMs"
        ));
        assert!(is_reserved_payload_extra_key(
            "freebuffSessionPollTimeoutMs"
        ));
        assert!(!is_reserved_payload_extra_key("temperature"));
    }

    #[test]
    fn normalizes_bare_codebuff_base_url_to_www() {
        assert_eq!(
            normalize_base_url("https://codebuff.com"),
            "https://www.codebuff.com"
        );
        assert_eq!(
            normalize_base_url("https://codebuff.com/api"),
            "https://www.codebuff.com/api"
        );
        assert_eq!(
            normalize_base_url("https://www.codebuff.com"),
            "https://www.codebuff.com"
        );
    }

    #[test]
    fn freebuff_request_plan_normalizes_base_url_and_chat_surface() {
        let mut payload = make_payload();
        payload.chat_completions_path = Some("/api/v1/chat/completions".to_string());
        let req = make_responses_request();
        let plan = build_request_plan(&payload, &req, "z-ai/glm-5.1", true).unwrap();
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, "https://www.codebuff.com/api/v1/chat/completions");
        assert_eq!(plan.response_kind, EndpointKind::ChatCompletions);
        assert_eq!(plan.body.as_ref().unwrap()["model"], "z-ai/glm-5.1");
        assert_eq!(plan.body.as_ref().unwrap()["stream"], true);
    }

    #[test]
    fn plan_freebuff_rejects_unsupported_endpoint() {
        let mut req = make_request();
        req.endpoint_kind = EndpointKind::Embeddings;
        let err = build_request_plan(&make_payload(), &req, "z-ai/glm-5.1", false)
            .expect_err("freebuff should reject embeddings");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.code.as_deref(), Some("unsupported_freebuff_endpoint"));
    }

    #[test]
    fn plan_freebuff_compatible_url_and_body() {
        let mut payload = make_payload();
        payload.chat_completions_path = Some("/api/v1/chat/completions".to_string());
        payload.extra_body = Some(HashMap::from([(
            "freebuffAgentId".to_string(),
            json!("base2-free"),
        )]));
        let req = make_responses_request();
        let plan = build_request_plan(&payload, &req, "z-ai/glm-5.1", true).unwrap();
        assert_freebuff_chat_surface_plan(&plan);
    }

    #[test]
    fn resolves_agent_from_model_map() {
        let payload = make_payload();
        let config = FreeBuffRuntimeConfig::from_payload(&payload, "z-ai/glm-5.1").unwrap();
        assert_eq!(config.agent_id, "base2-free");
        assert_eq!(config.rotation_interval, Duration::from_secs(900));
    }

    #[test]
    fn request_body_injects_codebuff_metadata() {
        let request = make_request();
        let body = build_chat_request_body(
            &request,
            "z-ai/glm-5.1",
            true,
            "run-123",
            "free",
            Some("instance-123"),
        );
        assert_eq!(body["model"], "z-ai/glm-5.1");
        assert_eq!(body["stream"], true);
        assert_eq!(body["codebuff_metadata"]["run_id"], "run-123");
        assert_eq!(body["codebuff_metadata"]["cost_mode"], "free");
        assert_eq!(
            body["codebuff_metadata"]["freebuff_instance_id"],
            "instance-123"
        );
        assert!(body["codebuff_metadata"]["client_id"]
            .as_str()
            .is_some_and(|value| value.len() == 13));
    }

    #[test]
    fn request_body_flattens_json_parts_for_freebuff_chat_surface() {
        let mut request = make_request();
        request.messages.push(CanonicalMessage {
            role: MessageRole::Tool,
            content: vec![ContentPart::Json {
                value: json!({
                    "city": "Hangzhou",
                    "condition": "sunny"
                }),
            }],
            name: None,
            tool_calls: Vec::new(),
            tool_call_id: Some("toolu_weather".to_string()),
        });

        let body = build_chat_request_body(
            &request,
            "z-ai/glm-5.1",
            false,
            "run-123",
            "free",
            Some("instance-123"),
        );
        assert_eq!(
            body["messages"][1]["content"],
            json!("{\"city\":\"Hangzhou\",\"condition\":\"sunny\"}")
        );
    }

    #[test]
    fn parses_active_freebuff_session_snapshot() {
        let config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
        let snapshot = parse_free_session_snapshot(
            &json!({
                "status": "active",
                "instanceId": "inst-active",
                "remainingMs": 3600000,
                "admittedAt": "2026-04-20T00:00:00.000Z",
                "expiresAt": "2026-04-20T01:00:00.000Z"
            }),
            &config,
        )
        .unwrap();
        assert!(matches!(snapshot.state, FreeBuffSessionState::Active));
        assert_eq!(snapshot.instance_id.as_deref(), Some("inst-active"));
        assert!(snapshot.is_fresh());
    }

    #[test]
    fn parses_queued_freebuff_session_snapshot() {
        let config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
        let snapshot = parse_free_session_snapshot(
            &json!({
                "status": "queued",
                "instanceId": "inst-queued",
                "position": 3,
                "queueDepth": 10,
                "estimatedWaitMs": 4200
            }),
            &config,
        )
        .unwrap();
        assert!(matches!(snapshot.state, FreeBuffSessionState::Queued));
        assert_eq!(snapshot.instance_id.as_deref(), Some("inst-queued"));
        assert_eq!(snapshot.queue_position, Some(3));
        assert_eq!(snapshot.queue_depth, Some(10));
        assert_eq!(snapshot.estimated_wait_ms, Some(4_200));
    }

    #[test]
    fn classifies_waiting_room_rejections() {
        assert_eq!(
            classify_waiting_room_rejection(
                426,
                r#"{"error":{"code":"freebuff_update_required","message":"missing instance"}}"#
            ),
            Some(FreeBuffWaitingRoomRejection::MissingInstance)
        );
        assert_eq!(
            classify_waiting_room_rejection(
                428,
                r#"{"error":{"code":"waiting_room_required","message":"waiting room required"}}"#
            ),
            Some(FreeBuffWaitingRoomRejection::WaitingRoomRequired)
        );
        assert_eq!(
            classify_waiting_room_rejection(
                429,
                r#"{"error":{"code":"waiting_room_queued","message":"queued"}}"#
            ),
            Some(FreeBuffWaitingRoomRejection::WaitingRoomQueued)
        );
        assert_eq!(
            classify_waiting_room_rejection(
                409,
                r#"{"error":{"code":"session_superseded","message":"superseded"}}"#
            ),
            Some(FreeBuffWaitingRoomRejection::SessionSuperseded)
        );
        assert_eq!(
            classify_waiting_room_rejection(
                410,
                r#"{"error":{"code":"session_expired","message":"expired"}}"#
            ),
            Some(FreeBuffWaitingRoomRejection::SessionExpired)
        );
    }

    #[test]
    fn detects_invalid_run_errors() {
        assert!(should_retry_with_fresh_run(
            400,
            r#"{"message":"runId not found"}"#
        ));
        assert!(should_retry_with_fresh_run(
            400,
            r#"{"error":{"message":"runId not running"}}"#
        ));
        assert!(!should_retry_with_fresh_run(429, "runId not found"));
    }

    #[test]
    fn auth_error_is_marked_as_token_invalidated() {
        let error = classify_auth_error("Unauthorized");
        assert_eq!(error.http_status, Some(401));
        assert!(error.message.contains("token_invalidated"));
        assert_eq!(error.code.as_deref(), Some("token_invalidated"));
    }
}
