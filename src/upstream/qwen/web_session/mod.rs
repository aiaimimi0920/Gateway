//! Typed current consumption-site owner; legacy refresh/flattening is not this channel's contract.
use crate::{
    error::{ErrorKind, FallbackHint, GatewayError},
    routing::candidate::ProviderAccountPayload,
};

pub(crate) fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.adapter == "qwen_web_compatible"
        && payload.call_flow.is_some_and(|flow| {
            flow.special_adapter == crate::protocol::call_flow::SpecialAdapter::QwenWeb
        })
}
pub(super) fn failure(code: &'static str, kind: ErrorKind) -> GatewayError {
    let mut error = GatewayError::bad_request(format!(
        "Qwen Web request failed: {code}; no replay or channel fallback."
    ));
    error.code = Some(code.into());
    error.kind = kind;
    error.http_status = Some(match kind {
        ErrorKind::BadRequest => 400,
        ErrorKind::Authentication => 401,
        ErrorKind::Timeout => 504,
        ErrorKind::ServiceUnavailable => 503,
        _ => 502,
    });
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: code.into(),
    };
    error.provider_name = Some("qwen-web".into());
    error
}
#[cfg(feature = "line-qwen-web-reverse")]
mod events;
#[cfg(feature = "line-qwen-web-reverse")]
mod material;
#[cfg(feature = "line-qwen-web-reverse")]
mod request;
#[cfg(feature = "line-qwen-web-reverse")]
mod transport;
#[cfg(feature = "line-qwen-web-reverse")]
pub(crate) use transport::{execute, execute_stream};
#[cfg(not(feature = "line-qwen-web-reverse"))]
mod disabled;
#[cfg(not(feature = "line-qwen-web-reverse"))]
pub(crate) use disabled::{execute, execute_stream};
#[cfg(all(test, feature = "line-qwen-web-reverse"))]
mod admission_tests;
#[cfg(all(test, feature = "line-qwen-web-reverse"))]
mod decoder_tests;
#[cfg(all(test, feature = "line-qwen-web-reverse"))]
pub(crate) mod test_support;
#[cfg(all(test, feature = "line-qwen-web-reverse"))]
mod tests;
