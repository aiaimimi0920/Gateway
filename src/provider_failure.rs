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
    let haystack = normalize_failure_text(error_code, error_message);

    if contains_any(
        &haystack,
        &[
            "invalidapikey",
            "invalidapi",
            "invalidtoken",
            "unauthorized",
            "incorrectapikey",
            "badcredentials",
            "credentialinvalid",
        ],
    ) || matches!(upstream_status, Some(401) | Some(403))
        && contains_any(
            &haystack,
            &["key", "token", "credential", "auth", "unauthorized"],
        )
    {
        return ProviderFailureClassification {
            class: ProviderFailureClass::CredentialInvalid,
            scope: ProviderFailureScope::Credential,
            permanent: true,
        };
    }

    if contains_any(
        &haystack,
        &[
            "expiredtoken",
            "tokenexpired",
            "expiredcredential",
            "sessionexpired",
        ],
    ) {
        return ProviderFailureClassification {
            class: ProviderFailureClass::CredentialExpired,
            scope: ProviderFailureScope::Credential,
            permanent: false,
        };
    }

    if contains_any(
        &haystack,
        &[
            "modelnotfound",
            "modeldoesnotexist",
            "modelunsupported",
            "unsupportedmodel",
            "unknownmodel",
        ],
    ) || matches!(upstream_status, Some(404)) && haystack.contains("model")
    {
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
