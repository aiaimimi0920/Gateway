use crate::error::GatewayError;
use crate::protocol::chatgpt::web_reverse as surface;

pub use crate::upstream::common::insert_header_map_value;

/// Fixed labels only: never include configured URLs, headers or response bodies.
pub enum ResponseStage {
    Bootstrap,
    Requirements,
    RequirementsPrepare,
    RequirementsFinalize,
    Prepare,
    Conversation,
}

impl ResponseStage {
    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::Bootstrap => "bootstrap",
            Self::Requirements => "requirements",
            Self::RequirementsPrepare => "requirements_prepare",
            Self::RequirementsFinalize => "requirements_finalize",
            Self::Prepare => "prepare",
            Self::Conversation => "conversation",
        }
    }
}

pub fn classify_chatgpt_web_text_response(
    stage: ResponseStage,
    status: u16,
    content_type: Option<&str>,
    body_text: &str,
) -> Result<(), GatewayError> {
    if !(200..300).contains(&status)
        || surface::response_indicates_browser_challenge(status, content_type, body_text)
        || surface::response_indicates_session_invalid(status, content_type, body_text)
    {
        let mut error = surface::classify_chatgpt_web_http_error(status, content_type, body_text);
        error.message = format!(
            "ChatGPT Web reverse upstream failure: stage={} upstream_status={status}. {}",
            stage.label(),
            error.message
        );
        return Err(error);
    }
    Ok(())
}

pub fn is_event_stream_content_type(content_type: Option<&str>) -> bool {
    content_type
        .and_then(|value| value.split(';').next())
        .map(|media_type| {
            media_type
                .trim_matches([' ', '\t'])
                .eq_ignore_ascii_case("text/event-stream")
        })
        .unwrap_or(false)
}
