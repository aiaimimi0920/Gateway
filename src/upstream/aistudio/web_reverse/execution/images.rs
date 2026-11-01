//! Image generation/editing use the Playground session HTTP lane, never API-key fallback.
use std::collections::HashMap;

use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::{aistudio_web, canonical::CanonicalRelayRequest, gemini_canvas};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;
use crate::upstream::gemini::web_reverse as gemini_web_reverse_modular;

fn validate_options(raw: &Value) -> Result<(), GatewayError> {
    if raw.get("n").is_some_and(|n| n.as_u64() != Some(1))
        || [
            "mask",
            "quality",
            "style",
            "background",
            "output_format",
            "output_compression",
        ]
        .iter()
        .any(|key| raw.get(key).is_some())
    {
        return Err(GatewayError::bad_request("AI Studio HTTP images supports one output and no mask, quality, style, background or output-encoding controls.")
            .with_provider(aistudio_web::AISTUDIO_WEB_REVERSE_ADAPTER).with_code("unsupported_aistudio_http_image_option"));
    }
    Ok(())
}

impl UpstreamClient {
    pub(crate) async fn execute_aistudio_web_images(
        &self,
        _provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let provider = aistudio_web::AISTUDIO_WEB_REVERSE_ADAPTER;
        let config = aistudio_web::config_from_payload(payload)?;
        if !config.fixture_transport {
            validate_options(&req.raw_body)?;
        }
        let prompt = gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
            req,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        )
        .map_err(|error| error.with_provider(provider))?;
        let image = if config.fixture_transport {
            let image =
                aistudio_web::fixture_image().map_err(|error| error.with_provider(provider))?;
            gemini_canvas::GeminiCanvasImage {
                mime_type: image.mime_type,
                bytes: image.bytes,
            }
        } else {
            let response = self
                .execute_aistudio_http_content(
                    payload,
                    model,
                    &aistudio_web::build_image_request_body(req, model),
                    None,
                )
                .await?;
            gemini_canvas::extract_inline_image_from_generate_content_response(&response)
                .map_err(|error| error.with_provider(provider))?
        };
        gemini_canvas::build_openai_images_response_from_bytes(req, &prompt, &[image])
            .map_err(|error| error.with_provider(provider))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn aistudio_image_options_never_silently_discard_batch_or_mask() {
        assert!(validate_options(&json!({"n":1})).is_ok());
        for raw in [
            json!({"n":2}),
            json!({"n":"1"}),
            json!({"mask":"data:..."}),
            json!({"quality":"high"}),
        ] {
            assert!(validate_options(&raw).is_err());
        }
    }
}
