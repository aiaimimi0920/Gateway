use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::provider_failure::{
    ProviderFailureClass, ProviderFailureClassification, ProviderFailureScope,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialModelHealthStatus {
    Active,
    Degraded,
    Cooling,
    Blocked,
}

impl CredentialModelHealthStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Degraded => "degraded",
            Self::Cooling => "cooling",
            Self::Blocked => "blocked",
        }
    }
}

impl std::fmt::Display for CredentialModelHealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialModelHealthTransition {
    pub status: CredentialModelHealthStatus,
    pub cooldown_seconds: Option<i64>,
    pub permanent: bool,
    pub penalizes_candidate: bool,
    pub failure_class: Option<String>,
    pub failure_scope: Option<String>,
}

impl CredentialModelHealthTransition {
    pub fn cooldown_until(&self, now: OffsetDateTime) -> Option<OffsetDateTime> {
        self.cooldown_seconds
            .filter(|seconds| *seconds > 0)
            .map(|seconds| now + Duration::seconds(seconds))
    }
}

pub fn transition_for_success() -> CredentialModelHealthTransition {
    CredentialModelHealthTransition {
        status: CredentialModelHealthStatus::Active,
        cooldown_seconds: None,
        permanent: false,
        penalizes_candidate: false,
        failure_class: None,
        failure_scope: None,
    }
}

pub fn transition_for_failure(
    classification: &ProviderFailureClassification,
) -> CredentialModelHealthTransition {
    let (status, cooldown_seconds, penalizes_candidate) = match classification.class {
        ProviderFailureClass::CredentialInvalid => {
            (CredentialModelHealthStatus::Blocked, None, true)
        }
        ProviderFailureClass::ModelUnsupported => {
            (CredentialModelHealthStatus::Blocked, None, true)
        }
        ProviderFailureClass::CredentialExpired => {
            (CredentialModelHealthStatus::Cooling, Some(15 * 60), true)
        }
        ProviderFailureClass::QuotaExhausted => {
            (CredentialModelHealthStatus::Cooling, Some(60 * 60), true)
        }
        ProviderFailureClass::RateLimited => {
            (CredentialModelHealthStatus::Cooling, Some(5 * 60), true)
        }
        ProviderFailureClass::ProviderTransient => {
            (CredentialModelHealthStatus::Degraded, Some(60), true)
        }
        ProviderFailureClass::GatewayProtocolError => {
            (CredentialModelHealthStatus::Degraded, Some(10 * 60), true)
        }
        ProviderFailureClass::ContentRejected | ProviderFailureClass::ClientRequestInvalid => {
            (CredentialModelHealthStatus::Active, None, false)
        }
        ProviderFailureClass::Unknown => match classification.scope {
            ProviderFailureScope::ClientRequest => {
                (CredentialModelHealthStatus::Active, None, false)
            }
            _ => (CredentialModelHealthStatus::Degraded, Some(60), true),
        },
    };

    CredentialModelHealthTransition {
        status,
        cooldown_seconds,
        permanent: classification.permanent,
        penalizes_candidate,
        failure_class: Some(classification.class_name().to_string()),
        failure_scope: Some(classification.scope_name().to_string()),
    }
}

pub fn candidate_penalty_status(status: &str, cooldown_until: Option<&str>) -> Option<String> {
    match status {
        "blocked" => Some("credential_model_blocked".to_string()),
        "cooling" => cooldown_until.map(|until| format!("credential_model_cooling_until:{until}")),
        "degraded" => Some("credential_model_degraded".to_string()),
        _ => None,
    }
}
