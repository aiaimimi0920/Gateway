//! Static AWS stream-failure identities, never upstream messages, credentials or replay grants.
use crate::{
    error::{ErrorKind, FallbackHint, GatewayError},
    protocol::stream_error::ProtocolStreamError,
};
use serde::Deserialize;
use std::borrow::Cow;

pub(super) const PROVIDER: &str = "bedrock_converse_compatible";

#[derive(Clone, Copy)]
pub(super) struct Exception {
    pub name: &'static str,
    code: &'static str,
    status: u16,
    kind: ErrorKind,
}

const MODELED: [Exception; 5] = [
    Exception {
        name: "internalServerException",
        code: "bedrock_internal_server_exception",
        status: 500,
        kind: ErrorKind::ServerError,
    },
    Exception {
        name: "modelStreamErrorException",
        code: "bedrock_model_stream_error_exception",
        status: 424,
        kind: ErrorKind::ServerError,
    },
    Exception {
        name: "validationException",
        code: "bedrock_validation_exception",
        status: 400,
        kind: ErrorKind::BadRequest,
    },
    Exception {
        name: "throttlingException",
        code: "bedrock_throttling_exception",
        status: 429,
        kind: ErrorKind::RateLimit,
    },
    Exception {
        name: "serviceUnavailableException",
        code: "bedrock_service_unavailable_exception",
        status: 503,
        kind: ErrorKind::ServiceUnavailable,
    },
];

impl Exception {
    pub fn from_error(error: &GatewayError) -> Option<Self> {
        (error.provider_name.as_deref() == Some(PROVIDER)).then_some(())?;
        MODELED
            .iter()
            .find(|value| Some(value.code) == error.code.as_deref())
            .copied()
    }
}

pub(super) fn failure(name: &str) -> ProtocolStreamError {
    let exception = MODELED.iter().find(|value| value.name == name);
    let safe = exception.map_or(
        match name {
            "malformed_model_output" => "malformed_model_output",
            "malformed_tool_use" => "malformed_tool_use",
            _ => "unknown_upstream_failure",
        },
        |value| value.name,
    );
    let mut error = GatewayError::server_error(format!("Bedrock stream failed: {safe}."))
        .with_code(exception.map_or("bedrock_stream_upstream_error", |value| value.code))
        .with_provider(PROVIDER);
    error.http_status = Some(exception.map_or(502, |value| value.status));
    error.kind = exception.map_or(ErrorKind::ServerError, |value| value.kind);
    // The HTTP exchange has already started. AWS's retry advice is not proof that replay is safe.
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Bedrock response already started; do not replay inference.".into(),
    };
    ProtocolStreamError::classified(format!("bedrock_stream_upstream_error: {safe}"), error)
}

/// Borrow ordinary JSON strings; escaped strings allocate only within the admitted frame budget.
/// Fields are optional in the AWS model; unknown members are skipped, not retained or forwarded.
#[derive(Deserialize)]
struct Payload<'a> {
    #[serde(default, borrow, rename = "message")]
    _message: Option<Cow<'a, str>>,
    #[serde(default, borrow, rename = "originalMessage")]
    _original_message: Option<Cow<'a, str>>,
    #[serde(default, rename = "originalStatusCode")]
    _original_status: Option<i32>,
}

pub(super) fn validate_payload(kind: &str, bytes: &[u8]) -> Result<(), ProtocolStreamError> {
    if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
        return Err(super::upstream::invalid(
            "exception payload is not an object",
        ));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| super::upstream::invalid("invalid exception UTF-8"))?;
    let valid = if kind == "modelStreamErrorException" {
        serde_json::from_str::<Payload<'_>>(text).map(|_| ())
    } else if MODELED.iter().any(|value| value.name == kind) {
        serde_json::from_str::<MessagePayload<'_>>(text).map(|_| ())
    } else {
        // A future shape has no known member contract; validate JSON but do not invent one.
        serde_json::from_str::<serde::de::IgnoredAny>(text).map(|_| ())
    };
    valid.map_err(|_| super::upstream::invalid("invalid exception payload"))
}

#[derive(Deserialize)]
struct MessagePayload<'a> {
    #[serde(default, borrow, rename = "message")]
    _message: Option<Cow<'a, str>>,
}
