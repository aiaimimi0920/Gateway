use crate::error::GatewayError;
use crate::protocol::chatgpt::web_reverse as surface;

pub use crate::upstream::common::insert_header_map_value;

pub fn classify_chatgpt_web_text_response(
    status: u16,
    content_type: Option<&str>,
    body_text: &str,
) -> Result<(), GatewayError> {
    if !(200..300).contains(&status)
        || surface::response_indicates_browser_challenge(status, content_type, body_text)
        || surface::response_indicates_session_invalid(status, content_type, body_text)
    {
        return Err(surface::classify_chatgpt_web_http_error(
            status,
            content_type,
            body_text,
        ));
    }
    Ok(())
}

pub fn is_event_stream_content_type(content_type: Option<&str>) -> bool {
    content_type
        .map(|value| value.to_ascii_lowercase().contains("text/event-stream"))
        .unwrap_or(false)
}
