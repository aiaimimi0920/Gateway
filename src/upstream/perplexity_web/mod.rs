//! Session identity stays available in every binary; the optional transport owns all network I/O.
mod session;
pub use session::Session;

pub const ASK_URL: &str = "https://www.perplexity.ai/rest/sse/perplexity_ask";
pub const MAX_DEADLINE: std::time::Duration = std::time::Duration::from_secs(120);

#[cfg(feature = "line-perplexity-web-reverse-api")]
mod exchange;
#[cfg(feature = "line-perplexity-web-reverse-api")]
mod live;
#[cfg(feature = "line-perplexity-web-reverse-api")]
mod transport;
#[cfg(feature = "line-perplexity-web-reverse-api")]
pub use live::execute_stream;
#[cfg(feature = "line-perplexity-web-reverse-api")]
pub use transport::{execute, execute_single_turn};
#[cfg(not(feature = "line-perplexity-web-reverse-api"))]
mod disabled;
#[cfg(not(feature = "line-perplexity-web-reverse-api"))]
pub use disabled::{execute, execute_single_turn, execute_stream};

pub(crate) fn unsupported_request_plan_error() -> crate::error::GatewayError {
    crate::protocol::perplexity_web::input_failure(
        "perplexity_web_request_plan_unsupported",
        "Perplexity Web requires its dedicated single-turn dispatcher; raw JSON/binary passthrough is unsupported.",
    )
}

#[cfg(all(test, feature = "line-perplexity-web-reverse-api"))]
pub(crate) mod test_support;

#[cfg(test)]
mod line_tests;
