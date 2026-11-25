use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFailureClass {
    CredentialInvalid,
    CredentialExpired,
    QuotaExhausted,
    RateLimited,
    ModelUnsupported,
    ContentRejected,
    GatewayProtocolError,
    ClientRequestInvalid,
    ProviderTransient,
    Unknown,
}

impl ProviderFailureClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CredentialInvalid => "credential_invalid",
            Self::CredentialExpired => "credential_expired",
            Self::QuotaExhausted => "quota_exhausted",
            Self::RateLimited => "rate_limited",
            Self::ModelUnsupported => "model_unsupported",
            Self::ContentRejected => "content_rejected",
            Self::GatewayProtocolError => "gateway_protocol_error",
            Self::ClientRequestInvalid => "client_request_invalid",
            Self::ProviderTransient => "provider_transient",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFailureScope {
    Credential,
    CredentialModel,
    Provider,
    ImplementationLine,
    ClientRequest,
    Unknown,
}

impl ProviderFailureScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Credential => "credential",
            Self::CredentialModel => "credential_model",
            Self::Provider => "provider",
            Self::ImplementationLine => "implementation_line",
            Self::ClientRequest => "client_request",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderFailureClassification {
    pub class: ProviderFailureClass,
    pub scope: ProviderFailureScope,
    pub permanent: bool,
}

impl ProviderFailureClassification {
    pub fn class_name(&self) -> &'static str {
        self.class.as_str()
    }

    pub fn scope_name(&self) -> &'static str {
        self.scope.as_str()
    }
}

pub fn classify_provider_failure(
    upstream_status: Option<u16>,
    error_code: Option<&str>,
    error_message: Option<&str>,
) -> ProviderFailureClassification {
    if upstream_status.is_some_and(|status| {
        crate::error::is_provider_ip_block_rejection(status, error_message.unwrap_or_default())
    }) {
        return ProviderFailureClassification {
            class: ProviderFailureClass::ProviderTransient,
            scope: ProviderFailureScope::Provider,
            permanent: false,
        };
    }
    if upstream_status.is_some_and(|status| {
        crate::error::is_model_capacity(status, error_code, error_message)
            || crate::error::is_model_pricing_failure(status, error_code, error_message)
            || crate::error::is_transient_distributor_unavailable(status, error_code, error_message)
    }) {
        return ProviderFailureClassification {
            class: ProviderFailureClass::ProviderTransient,
            scope: ProviderFailureScope::CredentialModel,
            permanent: false,
        };
    }
    // A known balance refusal is not an invalid key, even if its request-id contains "key".
    if upstream_status.is_some_and(|status| {
        crate::error::is_account_balance_rejection(status, error_message.unwrap_or_default())
    }) {
        return ProviderFailureClassification {
            class: ProviderFailureClass::QuotaExhausted,
            scope: ProviderFailureScope::Credential,
            permanent: false,
        };
    }
    let haystack = normalize_failure_text(error_code, error_message);

    // Check token/session expiry BEFORE generic 401/403 credential invalidation so
    // refreshable OAuth/session credentials enter Cooling rather than permanent Blocked.
    if contains_any(
        &haystack,
        &[
            "expiredtoken",
            "tokenexpired",
            "expiredcredential",
            "sessionexpired",
            "accesstokenexpired",
            "jwtisexpired",
        ],
    ) {
        return ProviderFailureClassification {
            class: ProviderFailureClass::CredentialExpired,
            scope: ProviderFailureScope::Credential,
            permanent: false,
        };
    }

    // Model-level entitlement or group-availability refusals must only scope to CredentialModel,
    // never permanently block the whole credential across other supported models.
    if contains_any(
        &haystack,
        &[
            "modelnotfound",
            "modeldoesnotexist",
            "modelunsupported",
            "unsupportedmodel",
            "unknownmodel",
            "isnotavailableforthisgroup",
            "notauthorizedtoaccessmodel",
            "doesnothaveaccesstomodel",
        ],
    ) {
        return ProviderFailureClassification {
            class: ProviderFailureClass::ModelUnsupported,
            scope: ProviderFailureScope::CredentialModel,
            permanent: true,
        };
    }

    if contains_any(
        &haystack,
        &[
            "invalidapikey",
            "invalidapi",
            "invalidtoken",
            "incorrectapikey",
            "badcredentials",
            "credentialinvalid",
            "tokenrevoked",
            "tokeninvalidated",
            "accountdisabled",
            "deactivatedworkspace",
        ],
    ) || matches!(upstream_status, Some(401))
        && contains_any(&haystack, &["apikey", "unauthorized", "credential"])
        || matches!(upstream_status, Some(403))
            && contains_any(
                &haystack,
                &[
                    "invalidapikey",
                    "incorrectapikey",
                    "badcredentials",
                    "revoked",
                    "suspended",
                ],
            )
    {
        return ProviderFailureClassification {
            class: ProviderFailureClass::CredentialInvalid,
            scope: ProviderFailureScope::Credential,
            permanent: true,
        };
    }

    if matches!(upstream_status, Some(404)) && haystack.contains("model") {
        return ProviderFailureClassification {
            class: ProviderFailureClass::ModelUnsupported,
            scope: ProviderFailureScope::CredentialModel,
            permanent: true,
        };
    }

    if contains_any(
        &haystack,
        &[
            "insufficientquota",
            "insufficientaccountbalance",
            "quotaexceeded",
            "quotaexhausted",
            "billinghardlimit",
            "creditsexhausted",
        ],
    ) || matches!(upstream_status, Some(402))
    {
        return ProviderFailureClassification {
            class: ProviderFailureClass::QuotaExhausted,
            scope: ProviderFailureScope::CredentialModel,
            permanent: false,
        };
    }

    if matches!(upstream_status, Some(429))
        || contains_any(&haystack, &["ratelimit", "toomanyrequests", "rateexceeded"])
    {
        return ProviderFailureClassification {
            class: ProviderFailureClass::RateLimited,
            scope: ProviderFailureScope::CredentialModel,
            permanent: false,
        };
    }

    if contains_any(
        &haystack,
        &[
            "contentfilter",
            "safety",
            "policyviolation",
            "blockedcontent",
        ],
    ) {
        return ProviderFailureClassification {
            class: ProviderFailureClass::ContentRejected,
            scope: ProviderFailureScope::ClientRequest,
            permanent: false,
        };
    }

    if contains_any(
        &haystack,
        &[
            "unsupportedcontenttype",
            "protocol",
            "invalidupstreamresponse",
            "parse",
            "schema",
        ],
    ) {
        return ProviderFailureClassification {
            class: ProviderFailureClass::GatewayProtocolError,
            scope: ProviderFailureScope::ImplementationLine,
            permanent: false,
        };
    }

    if matches!(upstream_status, Some(400) | Some(422)) {
        return ProviderFailureClassification {
            class: ProviderFailureClass::ClientRequestInvalid,
            scope: ProviderFailureScope::ClientRequest,
            permanent: false,
        };
    }

    if matches!(upstream_status, Some(500..=599)) {
        return ProviderFailureClassification {
            class: ProviderFailureClass::ProviderTransient,
            scope: ProviderFailureScope::Provider,
            permanent: false,
        };
    }

    ProviderFailureClassification {
        class: ProviderFailureClass::Unknown,
        scope: ProviderFailureScope::Unknown,
        permanent: false,
    }
}

fn normalize_failure_text(error_code: Option<&str>, error_message: Option<&str>) -> String {
    [error_code, error_message]
        .into_iter()
        .flatten()
        .flat_map(|value| value.chars())
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_invalid_key_takes_priority_over_generic_model_404() {
        let result = classify_provider_failure(
            Some(404),
            Some("invalid_api_key"),
            Some("Invalid API key for this model"),
        );
        assert_eq!(result.class, ProviderFailureClass::CredentialInvalid);
    }

    #[test]
    fn bulk_probe_ip_block_is_provider_scoped_not_a_bad_credential() {
        let classification = classify_provider_failure(
            Some(403),
            None,
            Some("该ip已被封禁，原因：bulk probe guard: ip 27.47.35.168 requested 5 distinct models in 60s (last_user 11552, offense 1)"),
        );
        assert_eq!(
            classification.class,
            ProviderFailureClass::ProviderTransient
        );
        assert_eq!(classification.scope, ProviderFailureScope::Provider);
        assert!(!classification.permanent);
    }

    #[test]
    fn expired_bearer_token_on_401_is_transient_not_permanent_invalid() {
        let classification = classify_provider_failure(
            Some(401),
            Some("token_expired"),
            Some("Unauthorized: Bearer access token expired"),
        );
        assert_eq!(
            classification.class,
            ProviderFailureClass::CredentialExpired
        );
        assert_eq!(classification.scope, ProviderFailureScope::Credential);
        assert!(!classification.permanent);
    }

    #[test]
    fn generic_invalid_response_wording_on_401_does_not_permanently_burn_session_credential() {
        let classification = classify_provider_failure(
            Some(401),
            Some("lobsterai_upstream_rejected"),
            Some("LobsterAI returned a failed or invalid Chat response"),
        );
        assert!(!classification.permanent);
    }

    #[test]
    fn transient_distributor_channel_unavailable_with_model_not_found_is_not_permanent() {
        for (status, code, message) in [
            (
                503,
                "model_not_found",
                "No available channel for model deepseek-v4.1-flash under group auto (distributor)",
            ),
            (
                502,
                "upstream_stream_model_not_found",
                "model_not_found: 分组 default 下模型 deepseek-v4.1-flash 无可用渠道（distributor）",
            ),
        ] {
            let classification = classify_provider_failure(Some(status), Some(code), Some(message));
            assert_eq!(
                classification.class,
                ProviderFailureClass::ProviderTransient
            );
            assert_eq!(classification.scope, ProviderFailureScope::CredentialModel);
            assert!(!classification.permanent);
        }
    }
}
