//! FreeBuff payload configuration, agent selection and credential bucket identity.
use super::{
    normalize_base_url, normalize_lookup_key, FREEBUFF_DEFAULT_AGENT_RUNS_PATH,
    FREEBUFF_DEFAULT_CHAT_COMPLETIONS_PATH, FREEBUFF_DEFAULT_COST_MODE,
    FREEBUFF_DEFAULT_ROTATION_SECS, FREEBUFF_DEFAULT_SESSION_PATH,
    FREEBUFF_DEFAULT_SESSION_POLL_INTERVAL_MS, FREEBUFF_DEFAULT_SESSION_POLL_TIMEOUT_MS,
    FREEBUFF_DEFAULT_USER_AGENT,
};
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use rand::seq::SliceRandom;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Clone)]
pub(super) struct FreeBuffRuntimeConfig {
    pub(super) bucket_key: String,
    pub(super) session_bucket_key: String,
    pub(super) base_url: String,
    pub(super) auth_token: String,
    pub(super) agent_id: String,
    pub(super) chat_completions_path: String,
    pub(super) start_run_path: String,
    pub(super) finish_run_path: String,
    pub(super) session_path: String,
    pub(super) rotation_interval: Duration,
    pub(super) session_poll_interval: Duration,
    pub(super) session_poll_timeout: Duration,
    pub(super) cost_mode: String,
    pub(super) user_agent: String,
}

impl std::fmt::Debug for FreeBuffRuntimeConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // URLs and custom paths can also contain credentials; omit all connection details.
        formatter
            .debug_struct("FreeBuffRuntimeConfig")
            .field("auth_token", &"[REDACTED_SECRET]")
            .finish_non_exhaustive()
    }
}

impl FreeBuffRuntimeConfig {
    pub(super) fn from_payload(
        payload: &ProviderAccountPayload,
        model: &str,
    ) -> Result<Self, GatewayError> {
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

    pub(super) fn requires_free_session(&self) -> bool {
        self.cost_mode.trim().eq_ignore_ascii_case("free")
    }
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

pub(super) fn extract_agent_id(value: &Value) -> Option<String> {
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

pub(super) fn read_payload_object<'a>(
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

pub(super) fn credential_subject_id(payload: &ProviderAccountPayload) -> String {
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
