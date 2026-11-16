//! Current consumption-site WS owner; explicit typed sessions never use legacy NDJSON.
use crate::{
    error::{ErrorKind, FallbackHint, GatewayError},
    routing::candidate::ProviderAccountPayload,
};

pub(crate) fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.adapter == "grok_compatible"
        && payload.call_flow.is_some_and(|flow| {
            flow.special_adapter == crate::protocol::call_flow::SpecialAdapter::GrokWeb
        })
}

pub(crate) fn failure(code: &'static str, kind: ErrorKind) -> GatewayError {
    let mut error = GatewayError::bad_request(format!(
        "Grok Web request failed: {code}; no replay or channel fallback."
    ));
    error.code = Some(code.into());
    error.kind = kind;
    error.http_status = Some(if kind == ErrorKind::BadRequest {
        400
    } else {
        502
    });
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: code.into(),
    };
    error.provider_name = Some("grok-web".into());
    error
}

pub(crate) fn unsupported_plan() -> GatewayError {
    failure("grok_web_request_plan_unsupported", ErrorKind::BadRequest)
}

#[cfg(feature = "line-grok-web-reverse-api")]
mod dial;
#[cfg(feature = "line-grok-web-reverse-api")]
mod machine;
#[cfg(feature = "line-grok-web-reverse-api")]
mod request;
#[cfg(feature = "line-grok-web-reverse-api")]
mod session;
#[cfg(feature = "line-grok-web-reverse-api")]
mod transport;
#[cfg(feature = "line-grok-web-reverse-api")]
pub(crate) use transport::{execute, execute_stream};

#[cfg(not(feature = "line-grok-web-reverse-api"))]
mod disabled;
#[cfg(not(feature = "line-grok-web-reverse-api"))]
pub(crate) use disabled::{execute, execute_stream};

#[cfg(all(test, feature = "line-grok-web-reverse-api"))]
mod boundary_tests;
#[cfg(all(test, feature = "line-grok-web-reverse-api"))]
mod machine_tests;
#[cfg(all(test, feature = "line-grok-web-reverse-api"))]
pub(crate) mod test_support;
#[cfg(all(test, feature = "line-grok-web-reverse-api"))]
mod tests;
