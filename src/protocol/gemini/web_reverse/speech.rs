//! Web read-aloud exports literal input, not model-generated text or a named TTS voice.
use serde_json::{json, Value};

use crate::error::{FallbackHint, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::gemini_canvas;

use super::{GeminiWebBootstrap, GeminiWebRequest};

pub(crate) const SPEECH_RPC: &str = "XqA3Ic";
pub(crate) const SPEECH_RPC_PATH: &str = "/_/BardChatUi/data/batchexecute";
pub(crate) const MAX_SPEECH_WIRE_BYTES: usize = 12 * 1024 * 1024;
pub(crate) const MAX_SPEECH_AUDIO_BYTES: usize = 8 * 1024 * 1024;

pub(crate) struct SpeechInput<'a> {
    pub text: &'a str,
    pub format: &'a str,
}

pub(crate) fn without_speech_replay(mut error: GatewayError) -> GatewayError {
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Gemini Web speech export must not be replayed automatically.".into(),
    };
    error
}

fn invalid(message: &str) -> GatewayError {
    without_speech_replay(
        GatewayError::bad_request(message)
            .with_provider("gemini_web_compatible")
            .with_code("unsupported_gemini_web_speech_request"),
    )
}

pub(crate) fn validate_speech<'a>(
    req: &'a CanonicalRelayRequest,
    model: &str,
) -> Result<SpeechInput<'a>, GatewayError> {
    let text = req.raw_body.get("input").and_then(Value::as_str);
    let text = text.filter(|text| !text.trim().is_empty() && text.len() <= 16 * 1024);
    let format = match req.raw_body.get("response_format") {
        None => Some("ogg"),
        Some(value) => value
            .as_str()
            .filter(|s| matches!(*s, "ogg" | "opus" | "wav")),
    };
    let unsupported = req.endpoint_kind != EndpointKind::AudioSpeech
        || req.stream
        || !matches!(model, "" | "gemini-web-chat")
        || req
            .raw_body
            .get("stream")
            .is_some_and(|v| v != &json!(false))
        || req
            .raw_body
            .get("speed")
            .is_some_and(|v| v.as_f64() != Some(1.0))
        || req
            .raw_body
            .get("voice")
            .is_some_and(|v| v.as_str() != Some("default"))
        || req
            .raw_body
            .get("instructions")
            .is_some_and(|v| !v.is_null() && v.as_str() != Some(""))
        || req
            .raw_body
            .get("stream_format")
            .is_some_and(|v| v.as_str() != Some("audio"))
        || req.raw_body.as_object().is_none_or(|body| {
            body.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "model"
                        | "input"
                        | "response_format"
                        | "voice"
                        | "speed"
                        | "instructions"
                        | "user"
                        | "stream"
                        | "stream_format"
                )
            })
        });
    if unsupported || text.is_none() || format.is_none() {
        return Err(invalid("Gemini Web speech requires 1-16384 UTF-8 bytes of literal input, model gemini-web-chat, the default voice at speed 1 and ogg/opus or native wav output; instructions and streaming are unsupported."));
    }
    Ok(SpeechInput {
        text: text.unwrap(),
        format: format.unwrap(),
    })
}

pub(crate) fn speech_export_request(
    input: &SpeechInput<'_>,
    bootstrap: &GeminiWebBootstrap,
) -> Result<GeminiWebRequest, GatewayError> {
    let locale = bootstrap.language.trim();
    // Pass the selected locale literally; script guessing can change the spoken language.
    if locale.len() > 35
        || locale.is_empty()
        || !locale
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err(invalid(
            "Gemini Web speech requires a valid session language tag.",
        ));
    }
    gemini_canvas::build_text_batchexecute_request(
        SPEECH_RPC,
        json!([null, input.text, locale, null, 2]),
        bootstrap,
        "/app",
    )
}

pub(crate) fn invalid_speech_response() -> GatewayError {
    without_speech_replay(
        GatewayError::server_error("Gemini Web speech returned no complete, RPC-bound audio in the requested native format; private response content was omitted.")
            .with_provider("gemini_web_compatible")
            .with_code("invalid_gemini_web_speech_response"),
    )
}
