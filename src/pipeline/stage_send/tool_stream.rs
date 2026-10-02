use std::pin::Pin;

use bytes::Bytes;
use futures::Stream;
use serde_json::Value;
use tracing::debug;

use crate::protocol::canonical::{CanonicalTool, EndpointKind};
use crate::protocol::stream_error::ProtocolStreamError;
use crate::protocol::tool_inject;

pub(super) type ByteStream<E = rquest::Error> =
    Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ToolDetectionPlacement {
    Disabled,
    RawOpenAi,
    NormalizedOpenAi,
}

impl ToolDetectionPlacement {
    pub(super) fn on_raw_openai(self) -> bool {
        self == Self::RawOpenAi
    }

    pub(super) fn after_normalization(self) -> bool {
        self == Self::NormalizedOpenAi
    }
}

pub(super) fn tool_detection_placement(
    endpoint_kind: EndpointKind,
    adapter: &str,
    uses_openai_responses_bridge: bool,
    tools_were_injected: bool,
) -> ToolDetectionPlacement {
    if !tools_were_injected {
        return ToolDetectionPlacement::Disabled;
    }

    if adapter == "anthropic_compatible"
        || uses_openai_responses_bridge
        || matches!(
            endpoint_kind,
            EndpointKind::Messages | EndpointKind::Responses
        )
    {
        ToolDetectionPlacement::NormalizedOpenAi
    } else {
        ToolDetectionPlacement::RawOpenAi
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn wrap_injected_openai_stream<E>(
    byte_stream: ByteStream<E>,
    detect_tools: bool,
    req_id: &uuid::Uuid,
    model: &str,
    original_tools: Vec<CanonicalTool>,
    original_tool_choice: Option<Value>,
    original_messages_text: Option<String>,
) -> ByteStream<E>
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    if !detect_tools {
        return byte_stream;
    }

    let resp_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    debug!(
        req_id = %req_id,
        "wrapping normalized OpenAI stream with XML tool call detector"
    );
    tool_inject::wrap_streaming_tool_detection_with_error(
        byte_stream,
        model.to_string(),
        resp_id,
        original_tools,
        original_tool_choice,
        original_messages_text,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_is_disabled_when_tools_were_not_injected() {
        for endpoint in [
            EndpointKind::ChatCompletions,
            EndpointKind::Completions,
            EndpointKind::Messages,
            EndpointKind::Responses,
        ] {
            assert_eq!(
                tool_detection_placement(endpoint, "openai_compatible", false, false),
                ToolDetectionPlacement::Disabled
            );
        }
    }

    #[test]
    fn plain_openai_chat_and_completions_detect_on_raw_stream() {
        for endpoint in [EndpointKind::ChatCompletions, EndpointKind::Completions] {
            let placement = tool_detection_placement(endpoint, "openai_compatible", false, true);
            assert!(placement.on_raw_openai());
            assert!(!placement.after_normalization());
        }
    }

    #[test]
    fn responses_upstreams_detect_only_after_normalization() {
        for endpoint in [EndpointKind::ChatCompletions, EndpointKind::Completions] {
            let placement = tool_detection_placement(endpoint, "openai_compatible", true, true);
            assert!(!placement.on_raw_openai());
            assert!(placement.after_normalization());
        }
    }

    #[test]
    fn endpoint_translation_detects_only_after_openai_normalization() {
        for endpoint in [EndpointKind::Messages, EndpointKind::Responses] {
            let placement = tool_detection_placement(endpoint, "openai_compatible", false, true);
            assert!(!placement.on_raw_openai());
            assert!(placement.after_normalization());
        }
    }

    #[test]
    fn anthropic_stream_detects_only_after_openai_normalization() {
        for endpoint in [EndpointKind::ChatCompletions, EndpointKind::Responses] {
            let placement = tool_detection_placement(endpoint, "anthropic_compatible", false, true);
            assert!(!placement.on_raw_openai());
            assert!(placement.after_normalization());
        }
    }
}
