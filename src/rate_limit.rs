use std::collections::HashMap;
use std::fmt::Write;
use std::time::Duration;

use async_trait::async_trait;
use redis::Script;
use sha2::{Digest, Sha256};

use crate::db::{GatewayRateLimitDefinition, GatewayRoutePolicyConfig};
use crate::error::GatewayError;
use crate::protocol::canonical::EndpointKind;
use crate::redis::pool::RedisPool;

const FIXED_WINDOW_SCRIPT: &str = r#"
local rule_count = #KEYS

for index = 1, rule_count do
  local argument_index = ((index - 1) * 2) + 1
  local max_requests = tonumber(ARGV[argument_index])
  local window_millis = tonumber(ARGV[argument_index + 1])
  local current = tonumber(redis.call('GET', KEYS[index]) or '0')

  if current >= max_requests then
    local retry_after_millis = redis.call('PTTL', KEYS[index])
    if retry_after_millis == -2 then
      -- The key expired between GET and PTTL. Treat it as a fresh window.
    elseif retry_after_millis == -1 then
      redis.call('PEXPIRE', KEYS[index], window_millis)
      retry_after_millis = window_millis
      return {0, index, retry_after_millis}
    else
      if retry_after_millis < 1 then
        retry_after_millis = 1
      end
      return {0, index, retry_after_millis}
    end
  end
end

for index = 1, rule_count do
  local argument_index = ((index - 1) * 2) + 1
  local window_millis = tonumber(ARGV[argument_index + 1])
  local current = redis.call('INCR', KEYS[index])
  if current == 1 or redis.call('PTTL', KEYS[index]) < 0 then
    redis.call('PEXPIRE', KEYS[index], window_millis)
  end
end

return {1, 0, 0}
"#;
const DEFAULT_RATE_LIMIT_STORE_TIMEOUT: Duration = Duration::from_millis(750);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum RateLimitScope {
    Project,
    AccessKey,
    Model,
    Endpoint,
    ProviderAttempt,
}

impl RateLimitScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::AccessKey => "access_key",
            Self::Model => "model",
            Self::Endpoint => "endpoint",
            Self::ProviderAttempt => "provider_attempt",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateLimitScopeKey {
    pub scope: RateLimitScope,
    pub key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateLimitRule {
    pub scope: RateLimitScope,
    pub key: String,
    pub window_seconds: u64,
    pub max_requests: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct RateLimitDimensions<'a> {
    pub tenant_id: &'a str,
    pub project_id: &'a str,
    pub access_key_id: Option<&'a str>,
    pub model: Option<&'a str>,
    pub endpoint: &'a str,
    pub endpoint_policy_alias: Option<&'a str>,
    pub provider_account_id: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RateLimitBackendDecision {
    Allowed,
    Rejected {
        rule_index: usize,
        retry_after_millis: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RateLimitAdmission {
    Allowed,
    Rejected {
        scope: RateLimitScope,
        retry_after_millis: u64,
    },
    BypassedStoreUnavailable {
        reason: String,
    },
    IndeterminateStoreFailure {
        reason: String,
    },
}

impl RateLimitAdmission {
    pub fn into_gateway_result(self) -> Result<(), GatewayError> {
        match self {
            Self::Allowed | Self::BypassedStoreUnavailable { .. } => Ok(()),
            Self::IndeterminateStoreFailure { .. } => Err(GatewayError::service_unavailable(
                "rate-limit admission result is indeterminate",
            )
            .with_code("rate_limit_admission_indeterminate")),
            Self::Rejected {
                scope,
                retry_after_millis,
            } => Err(GatewayError::rate_limited(
                format!("rate limit exceeded for {} scope", scope.as_str()),
                retry_after_millis.max(1),
            )
            .with_code("rate_limit_exceeded")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RateLimitStoreError {
    Unavailable(String),
    Indeterminate(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderRateLimitRejections {
    minimum_retry_after_millis: Option<u64>,
}

impl ProviderRateLimitRejections {
    pub fn observe(&mut self, retry_after_millis: u64) {
        let retry_after_millis = retry_after_millis.max(1);
        self.minimum_retry_after_millis = Some(
            self.minimum_retry_after_millis
                .map_or(retry_after_millis, |current| {
                    current.min(retry_after_millis)
                }),
        );
    }

    pub fn resolve_terminal_error(self, real_error: Option<GatewayError>) -> Option<GatewayError> {
        real_error.or_else(|| {
            self.minimum_retry_after_millis.map(|retry_after_millis| {
                GatewayError::rate_limited(
                    "rate limit exceeded for provider_attempt scope",
                    retry_after_millis,
                )
                .with_code("rate_limit_exceeded")
            })
        })
    }
}

#[async_trait]
pub trait RateLimitStore: Send + Sync {
    async fn admit(
        &self,
        rules: &[RateLimitRule],
    ) -> Result<RateLimitBackendDecision, RateLimitStoreError>;
}

pub struct RedisRateLimitStore<'a> {
    pool: &'a RedisPool,
}

impl<'a> RedisRateLimitStore<'a> {
    pub fn new(pool: &'a RedisPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RateLimitStore for RedisRateLimitStore<'_> {
    async fn admit(
        &self,
        rules: &[RateLimitRule],
    ) -> Result<RateLimitBackendDecision, RateLimitStoreError> {
        if rules.is_empty() {
            return Ok(RateLimitBackendDecision::Allowed);
        }

        let mut connection = self.pool.get().await.map_err(|_| {
            RateLimitStoreError::Unavailable("redis rate-limit connection unavailable".to_string())
        })?;
        let script = Script::new(FIXED_WINDOW_SCRIPT);
        let mut invocation = script.prepare_invoke();
        for rule in rules {
            invocation
                .key(&rule.key)
                .arg(rule.max_requests)
                .arg(rule.window_seconds.saturating_mul(1_000));
        }
        let response = invocation
            .invoke_async::<Vec<i64>>(&mut connection)
            .await
            .map_err(|_| {
                RateLimitStoreError::Indeterminate(
                    "redis rate-limit admission result is unknown".to_string(),
                )
            })?;
        if response.first().copied() == Some(1) {
            return Ok(RateLimitBackendDecision::Allowed);
        }

        let rule_index = response
            .get(1)
            .copied()
            .and_then(|value| usize::try_from(value).ok())
            .and_then(|value| value.checked_sub(1))
            .ok_or_else(|| {
                RateLimitStoreError::Indeterminate(
                    "redis rate-limit response was invalid".to_string(),
                )
            })?;
        let retry_after_millis = response
            .get(2)
            .copied()
            .and_then(|value| u64::try_from(value).ok())
            .unwrap_or(1);
        Ok(RateLimitBackendDecision::Rejected {
            rule_index,
            retry_after_millis: retry_after_millis.max(1),
        })
    }
}

pub async fn enforce_rate_limit_rules(
    store: &(impl RateLimitStore + ?Sized),
    rules: &[RateLimitRule],
) -> RateLimitAdmission {
    enforce_rate_limit_rules_with_timeout(store, rules, DEFAULT_RATE_LIMIT_STORE_TIMEOUT).await
}

pub async fn enforce_rate_limit_rules_with_timeout(
    store: &(impl RateLimitStore + ?Sized),
    rules: &[RateLimitRule],
    timeout_duration: Duration,
) -> RateLimitAdmission {
    if rules.is_empty() {
        return RateLimitAdmission::Allowed;
    }
    let decision = match tokio::time::timeout(timeout_duration, store.admit(rules)).await {
        Ok(decision) => decision,
        Err(_) => {
            return RateLimitAdmission::IndeterminateStoreFailure {
                reason: "rate-limit store admission timed out".to_string(),
            };
        }
    };
    match decision {
        Ok(RateLimitBackendDecision::Allowed) => RateLimitAdmission::Allowed,
        Ok(RateLimitBackendDecision::Rejected {
            rule_index,
            retry_after_millis,
        }) => rules.get(rule_index).map_or_else(
            || RateLimitAdmission::IndeterminateStoreFailure {
                reason: "rate-limit store returned an invalid rule index".to_string(),
            },
            |rule| RateLimitAdmission::Rejected {
                scope: rule.scope,
                retry_after_millis: retry_after_millis.max(1),
            },
        ),
        Err(RateLimitStoreError::Unavailable(reason)) => {
            RateLimitAdmission::BypassedStoreUnavailable { reason }
        }
        Err(RateLimitStoreError::Indeterminate(reason)) => {
            RateLimitAdmission::IndeterminateStoreFailure { reason }
        }
    }
}

pub fn build_request_rate_limit_rules(
    config: &GatewayRoutePolicyConfig,
    dimensions: &RateLimitDimensions<'_>,
) -> Result<Vec<RateLimitRule>, GatewayError> {
    if config.rate_limit_enforcement_version.as_deref() != Some("v1") {
        return Ok(Vec::new());
    }
    let keys = build_rate_limit_scope_keys(dimensions)?;
    let mut rules = Vec::new();

    if let Some(definition) = project_rate_limit_definition(config) {
        push_rule(&mut rules, &keys, RateLimitScope::Project, &definition);
    }
    if let Some(definition) = config.api_key_rate_limit.as_ref() {
        push_rule(&mut rules, &keys, RateLimitScope::AccessKey, definition);
    }
    if let Some(definition) =
        find_rate_limit_definition(config.model_rate_limits.as_ref(), [dimensions.model, None])
    {
        push_rule(&mut rules, &keys, RateLimitScope::Model, definition);
    }
    if let Some(definition) = find_rate_limit_definition(
        config.endpoint_rate_limits.as_ref(),
        [Some(dimensions.endpoint), dimensions.endpoint_policy_alias],
    ) {
        push_rule(&mut rules, &keys, RateLimitScope::Endpoint, definition);
    }

    Ok(rules)
}

pub fn build_provider_attempt_rate_limit_rule(
    config: &GatewayRoutePolicyConfig,
    dimensions: &RateLimitDimensions<'_>,
) -> Result<Option<RateLimitRule>, GatewayError> {
    if config.rate_limit_enforcement_version.as_deref() != Some("v1") {
        return Ok(None);
    }
    let Some(definition) = config.provider_attempt_rate_limit.as_ref() else {
        return Ok(None);
    };
    let keys = build_rate_limit_scope_keys(dimensions)?;
    Ok(rule_for_scope(
        &keys,
        RateLimitScope::ProviderAttempt,
        definition,
    ))
}

pub fn build_rate_limit_scope_keys(
    dimensions: &RateLimitDimensions<'_>,
) -> Result<Vec<RateLimitScopeKey>, GatewayError> {
    let tenant_id = required_component(dimensions.tenant_id, "tenant_id", false)?;
    let project_id = required_component(dimensions.project_id, "project_id", false)?;
    let endpoint = required_component(dimensions.endpoint, "endpoint", true)?;
    let namespace_hash = rate_limit_namespace_hash(&tenant_id, &project_id);
    let mut keys = vec![scope_key(
        RateLimitScope::Project,
        &namespace_hash,
        &[
            ("tenant", tenant_id.as_str()),
            ("project", project_id.as_str()),
        ],
    )];

    if let Some(access_key_id) =
        optional_component(dimensions.access_key_id, "access_key_id", false)?
    {
        keys.push(scope_key(
            RateLimitScope::AccessKey,
            &namespace_hash,
            &[
                ("tenant", tenant_id.as_str()),
                ("project", project_id.as_str()),
                ("access_key", access_key_id.as_str()),
            ],
        ));
    }

    if let Some(model) = optional_component(dimensions.model, "model", true)? {
        keys.push(scope_key(
            RateLimitScope::Model,
            &namespace_hash,
            &[
                ("tenant", tenant_id.as_str()),
                ("project", project_id.as_str()),
                ("model", model.as_str()),
            ],
        ));
    }

    keys.push(scope_key(
        RateLimitScope::Endpoint,
        &namespace_hash,
        &[
            ("tenant", tenant_id.as_str()),
            ("project", project_id.as_str()),
            ("endpoint", endpoint.as_str()),
        ],
    ));

    if let Some(provider_account_id) =
        optional_component(dimensions.provider_account_id, "provider_account_id", false)?
    {
        keys.push(scope_key(
            RateLimitScope::ProviderAttempt,
            &namespace_hash,
            &[
                ("tenant", tenant_id.as_str()),
                ("project", project_id.as_str()),
                ("provider", provider_account_id.as_str()),
            ],
        ));
    }

    Ok(keys)
}

pub fn endpoint_rate_limit_identity(endpoint_kind: EndpointKind) -> (&'static str, &'static str) {
    match endpoint_kind {
        EndpointKind::ChatCompletions => ("chat_completions", "post /v1/chat/completions"),
        EndpointKind::Completions => ("completions", "post /v1/completions"),
        EndpointKind::Embeddings => ("embeddings", "post /v1/embeddings"),
        EndpointKind::ImagesGenerations => ("images_generations", "post /v1/images/generations"),
        EndpointKind::ImagesEdits => ("images_edits", "post /v1/images/edits"),
        EndpointKind::MusicGenerations => ("music_generations", "post /v1/music/generations"),
        EndpointKind::VideosGenerations => ("videos_generations", "post /v1/videos/generations"),
        EndpointKind::AudioTranscriptions => {
            ("audio_transcriptions", "post /v1/audio/transcriptions")
        }
        EndpointKind::AudioSpeech => ("audio_speech", "post /v1/audio/speech"),
        EndpointKind::Messages => ("messages", "post /v1/messages"),
        EndpointKind::Responses => ("responses", "post /v1/responses"),
        EndpointKind::Search => ("search", "post /v1/search"),
        EndpointKind::Fetch => ("fetch", "post /v1/fetch"),
        EndpointKind::ResearchCreate => ("research_create", "post /v1/research"),
        EndpointKind::ResearchList => ("research_list", "get /v1/research"),
        EndpointKind::ResearchGet => ("research_get", "get /v1/research/:id"),
        EndpointKind::CreditsBalance => ("credits_balance", "get /v1/credits/balance"),
    }
}

fn project_rate_limit_definition(
    config: &GatewayRoutePolicyConfig,
) -> Option<GatewayRateLimitDefinition> {
    match (
        config.rate_limit_window_seconds,
        config.rate_limit_max_requests,
    ) {
        (Some(window_seconds), Some(max_requests)) if window_seconds > 0 && max_requests > 0 => {
            Some(GatewayRateLimitDefinition {
                window_seconds,
                max_requests,
            })
        }
        _ => None,
    }
}

fn find_rate_limit_definition<'a, const N: usize>(
    definitions: Option<&'a HashMap<String, GatewayRateLimitDefinition>>,
    candidates: [Option<&str>; N],
) -> Option<&'a GatewayRateLimitDefinition> {
    let definitions = definitions?;
    for candidate in candidates.into_iter().flatten() {
        let candidate = candidate.trim().to_ascii_lowercase();
        if candidate.is_empty() {
            continue;
        }
        if let Some(definition) = definitions.get(&candidate) {
            return Some(definition);
        }
        if let Some((_, definition)) = definitions
            .iter()
            .find(|(key, _)| key.trim().eq_ignore_ascii_case(&candidate))
        {
            return Some(definition);
        }
    }
    None
}

fn push_rule(
    rules: &mut Vec<RateLimitRule>,
    keys: &[RateLimitScopeKey],
    scope: RateLimitScope,
    definition: &GatewayRateLimitDefinition,
) {
    if let Some(rule) = rule_for_scope(keys, scope, definition) {
        rules.push(rule);
    }
}

fn rule_for_scope(
    keys: &[RateLimitScopeKey],
    scope: RateLimitScope,
    definition: &GatewayRateLimitDefinition,
) -> Option<RateLimitRule> {
    if definition.window_seconds <= 0 || definition.max_requests <= 0 {
        return None;
    }
    let key = keys.iter().find(|entry| entry.scope == scope)?;
    Some(RateLimitRule {
        scope,
        key: key.key.clone(),
        window_seconds: definition.window_seconds as u64,
        max_requests: definition.max_requests as u64,
    })
}

fn rate_limit_namespace_hash(tenant_id: &str, project_id: &str) -> String {
    let canonical = format!(
        "tenant:{}:{tenant_id};project:{}:{project_id};",
        tenant_id.len(),
        project_id.len()
    );
    format!("{:x}", Sha256::digest(canonical.as_bytes()))
}

fn scope_key(
    scope: RateLimitScope,
    namespace_hash: &str,
    components: &[(&str, &str)],
) -> RateLimitScopeKey {
    let mut canonical = String::new();
    for (label, value) in components {
        let _ = write!(&mut canonical, "{label}:{}:{value};", value.len());
    }
    let digest = Sha256::digest(canonical.as_bytes());
    RateLimitScopeKey {
        scope,
        key: format!(
            "gateway:rate-limit:v1:{{{namespace_hash}}}:{}:{digest:x}",
            scope.as_str()
        ),
    }
}

fn required_component(value: &str, label: &str, lowercase: bool) -> Result<String, GatewayError> {
    optional_component(Some(value), label, lowercase)?.ok_or_else(|| {
        GatewayError::bad_request(format!("rate-limit {label} must not be empty"))
            .with_code("rate_limit_scope_invalid")
    })
}

fn optional_component(
    value: Option<&str>,
    label: &str,
    lowercase: bool,
) -> Result<Option<String>, GatewayError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Err(
            GatewayError::bad_request(format!("rate-limit {label} must not be empty"))
                .with_code("rate_limit_scope_invalid"),
        );
    }
    Ok(Some(if lowercase {
        value.to_ascii_lowercase()
    } else {
        value.to_string()
    }))
}
