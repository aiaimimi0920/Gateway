use super::{normalize_lookup_key, FreeBuffRuntimeConfig};
use crate::error::GatewayError;
use serde_json::Value;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FreeBuffSessionState {
    Disabled,
    None,
    Queued,
    Active,
    Draining,
    Superseded,
    Expired,
}

#[derive(Debug, Clone)]
pub(super) struct FreeBuffSessionSnapshot {
    pub(super) state: FreeBuffSessionState,
    pub(super) instance_id: Option<String>,
    pub(super) queue_position: Option<u64>,
    pub(super) queue_depth: Option<u64>,
    pub(super) estimated_wait_ms: Option<u64>,
    _admitted_at: Option<String>,
    _expires_at: Option<String>,
    _grace_ends_at: Option<String>,
    pub(super) message: Option<String>,
    _observed_at: Instant,
    pub(super) refresh_after: Instant,
}

impl FreeBuffSessionSnapshot {
    pub(super) fn is_fresh(&self) -> bool {
        Instant::now() < self.refresh_after
    }

    pub(super) fn active_instance_id(&self) -> Option<&str> {
        match self.state {
            FreeBuffSessionState::Active => self.instance_id.as_deref(),
            _ => None,
        }
    }

    pub(super) fn retry_delay_ms(&self, fallback_ms: u64) -> u64 {
        self.estimated_wait_ms.unwrap_or(fallback_ms).max(250)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FreeBuffWaitingRoomRejection {
    MissingInstance,
    WaitingRoomRequired,
    WaitingRoomQueued,
    SessionSuperseded,
    SessionExpired,
}

pub(super) fn parse_free_session_snapshot(
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
        // An unrepresentable refresh deadline must not make cached state fresh forever.
        refresh_after: observed_at
            .checked_add(refresh_delay)
            .unwrap_or(observed_at),
    })
}

pub(super) fn build_synthetic_session_snapshot(
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
        refresh_after: observed_at
            .checked_add(refresh_delay)
            .unwrap_or(observed_at),
    }
}

pub(super) fn build_waiting_room_queued_error(
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

pub(super) fn classify_waiting_room_rejection(
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

pub(super) fn classify_waiting_room_error(
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

pub(super) fn should_retry_with_fresh_run(status: u16, body: &str) -> bool {
    if status != 400 {
        return false;
    }
    let lower = body.to_ascii_lowercase();
    lower.contains("runid not found") || lower.contains("runid not running")
}

pub(super) fn classify_auth_error(body: &str) -> GatewayError {
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
