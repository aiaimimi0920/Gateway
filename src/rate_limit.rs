mod memory;
mod rules;
pub use memory::MemoryRateLimitStore;

pub use rules::{
    build_provider_attempt_rate_limit_rule, build_rate_limit_scope_keys,
    build_request_rate_limit_rules, endpoint_rate_limit_identity,
};

use std::time::Duration;

use async_trait::async_trait;
use redis::Script;

use crate::error::GatewayError;
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
