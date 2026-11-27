//! Gateway-owned, pure-Rust remote execution; zero external Node or Cursor runtime.
mod chat_request;
mod cohere_request;
mod gemini_request;
mod gemini_tools;
#[cfg(test)]
mod http_tests;
mod incremental;
pub(crate) mod ingress;
mod inline_images;
pub(crate) mod metered_output;
mod metered_request;
pub(crate) mod native_output;
pub(crate) mod native_publication;
#[cfg(test)]
mod native_request_tests;
pub(crate) mod request;
pub(crate) mod runtime;
pub(crate) mod sse;
mod text_output;
pub(crate) mod thinking;
pub(crate) mod tool_request;
mod tool_schema;
pub(crate) mod upstream;

pub(crate) fn error(code: &str) -> crate::error::GatewayError {
    crate::routing::config::cursor::stop(
        crate::error::GatewayError::service_unavailable("Cursor remote execution did not complete")
            .with_code(format!("cursor_remote_{code}")),
    )
}
