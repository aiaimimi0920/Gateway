//! Speech uses Playground HTTP and local PCM/WAV/MP3 formatting, never browser/API fallback.
use std::collections::HashMap;

use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::{aistudio_web, canonical::CanonicalRelayRequest, gemini_canvas};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;
use crate::upstream::response_types::BinaryUpstreamResponse;

fn validate_options(raw: &Value) -> Result<(), GatewayError> {
    if raw
        .get("speed")
        .is_some_and(|speed| speed.as_f64() != Some(1.0))
        || raw.as_object().is_none_or(|object| {
            object.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "model" | "input" | "voice" | "response_format" | "speed" | "user"
                )
            })
        })
    {
        return Err(GatewayError::bad_request(
            "AI Studio HTTP speech does not implement speed or additional speech controls.",
        )
        .with_provider(aistudio_web::AISTUDIO_WEB_REVERSE_ADAPTER)
        .with_code("unsupported_aistudio_http_speech_option"));
    }
    Ok(())
}

impl UpstreamClient {
    pub(crate) async fn execute_aistudio_web_tts(
        &self,
        _provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        let provider = aistudio_web::AISTUDIO_WEB_REVERSE_ADAPTER;
        let config = aistudio_web::config_from_payload(payload)?;
        let (bytes, content_type) = if config.fixture_transport {
            aistudio_web::build_fixture_audio_binary_response(req)
                .map_err(|error| error.with_provider(provider))?
        } else {
            validate_options(&req.raw_body)?;
            let format = req
                .raw_body
                .get("response_format")
                .and_then(Value::as_str)
                .unwrap_or("wav");
            if !matches!(format, "wav" | "pcm" | "mp3") {
                return Err(GatewayError::bad_request(
                    "AI Studio HTTP speech supports wav, pcm, or mp3.",
                )
                .with_provider(provider)
                .with_code("unsupported_aistudio_http_audio_format"));
            }
            let response = self
                .execute_aistudio_http_content(
                    payload,
                    model,
                    &aistudio_web::build_tts_request_body(req, model),
                    Some(format),
                )
                .await?;
            let audio = gemini_canvas::extract_audio_from_generate_content_response(&response)
                .map_err(|error| error.with_provider(provider))?;
            (audio.bytes, audio.mime_type)
        };
        Ok(BinaryUpstreamResponse {
            body: bytes::Bytes::from(bytes),
            content_type: Some(content_type),
            extra_headers: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn aistudio_speech_rejects_unimplemented_controls_before_generation() {
        assert!(validate_options(&json!({"speed":1})).is_ok());
        assert!(validate_options(&json!({"speed":2})).is_err());
        assert!(validate_options(&json!({"instructions":"ignored"})).is_err());
    }
}
