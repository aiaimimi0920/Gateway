use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini_canvas::GeminiCanvasMediaOperation;

pub fn prompt_for_legacy_mixed_lane_media_request(
    req: &CanonicalRelayRequest,
    operation: GeminiCanvasMediaOperation,
) -> Result<String, GatewayError> {
    super::super::web_reverse_media_prompt::prompt_for_media_request(req, operation)
}
