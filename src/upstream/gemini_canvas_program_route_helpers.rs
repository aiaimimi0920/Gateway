use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::gemini_canvas;
use crate::routing::candidate::{ProviderAccountPayload, ProviderExecutionMode};
use crate::upstream::gemini::canvas_program_web_reverse as gemini_canvas_program_web_reverse_modular;

fn should_fail_closed_gemini_canvas_program_browserless(payload: &ProviderAccountPayload) -> bool {
    payload.adapter == "gemini_canvas_program_web_reverse_compatible"
}

pub(crate) fn gemini_canvas_program_payload_has_explicit_official_api_identity(
    payload: &ProviderAccountPayload,
) -> bool {
    if payload.adapter != "gemini_canvas_program_web_reverse_compatible" {
        return false;
    }
    if payload.api_key.trim().starts_with("AIza") {
        return true;
    }
    let Some(extra_body) = payload.extra_body.as_ref() else {
        return false;
    };
    for field_name in [
        "googleApiKey",
        "google_api_key",
        "cloudApiKey",
        "cloud_api_key",
    ] {
        if extra_body
            .get(field_name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
        {
            return true;
        }
    }
    extra_body
        .get("apiKeys")
        .or_else(|| extra_body.get("api_keys"))
        .and_then(Value::as_array)
        .map(|values| {
            values.iter().any(|value| {
                value
                    .as_str()
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .is_some()
            })
        })
        .unwrap_or(false)
}

pub(crate) fn ensure_gemini_canvas_program_payload_avoids_official_api_identity(
    payload: &ProviderAccountPayload,
) -> Result<(), GatewayError> {
    if gemini_canvas_program_payload_has_explicit_official_api_identity(payload) {
        return Err(GatewayError::bad_request(
            "Gemini Canvas program-owned default route forbids explicit official Google API keys. Keep official_api and canvas_web_reverse/program_owned separated.",
        )
        .with_provider("gemini_canvas_program_web_reverse_compatible")
        .with_code("gemini_canvas_program_official_api_key_forbidden"));
    }
    Ok(())
}

pub(crate) fn gemini_canvas_program_runtime_material_official_api_key_forbidden_error(
) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas program-owned runtime material still carries official Google API keys. Remove googleApiKey/apiKeys from the materialized browser-state before invoking this line.",
    )
    .with_provider("gemini_canvas_program_web_reverse_compatible")
    .with_code("gemini_canvas_program_official_api_key_forbidden")
}

pub(crate) fn gemini_canvas_program_tts_pure_http_required_error(provider: &str) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas program-owned TTS lane requires the pure HTTP page-owned Canvas program contract.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_tts_pure_http_required")
}

pub(crate) fn gemini_canvas_program_text_pure_http_required_error(provider: &str) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas program-owned text lane requires the pure HTTP page-owned Canvas program contract.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_text_pure_http_required")
}

pub(crate) fn gemini_canvas_program_image_pure_http_required_error(provider: &str) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas program-owned image lane requires the pure HTTP page-owned Canvas program contract.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_image_pure_http_required")
}

pub(crate) fn should_attempt_gemini_canvas_program_modular_media_direct_http(
    payload: &ProviderAccountPayload,
    operation: gemini_canvas::GeminiCanvasMediaOperation,
) -> bool {
    if payload.adapter != "gemini_canvas_program_web_reverse_compatible"
        || !gemini_canvas::pure_http_enabled(payload)
    {
        return false;
    }

    if operation != gemini_canvas::GeminiCanvasMediaOperation::Video {
        return true;
    }

    if gemini_canvas::pure_http_required(payload) {
        return true;
    }

    gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_has_concrete_handle(
        payload,
    )
}

pub(crate) fn should_treat_gemini_canvas_program_modular_media_direct_http_as_authoritative(
    payload: &ProviderAccountPayload,
) -> bool {
    should_fail_closed_gemini_canvas_program_browserless(payload)
        || matches!(
            payload.execution_mode,
            Some(ProviderExecutionMode::DirectHttp)
        )
        || gemini_canvas::pure_http_required(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    fn make_payload(adapter: &str) -> ProviderAccountPayload {
        serde_json::from_value(json!({
            "adapter": adapter,
            "baseUrl": "https://gemini.google.com",
            "apiKey": ""
        }))
        .expect("payload")
    }

    fn make_gemini_canvas_program_payload_with_handle(
        operation: Option<&str>,
    ) -> ProviderAccountPayload {
        let mut payload = make_payload("gemini_canvas_program_web_reverse_compatible");
        payload.runtime_state_object_key =
            Some("credential-runtime/gemini-canvas/program/storage-state.json".to_string());
        let mut extra = HashMap::from([
            ("shareId".to_string(), json!("canvas-share-789")),
            ("appPath".to_string(), json!("/app/4abc4e7577b6149f")),
            ("conversationId".to_string(), json!("c_4abc4e7577b6149f")),
            ("responseId".to_string(), json!("r_payload")),
        ]);
        if let Some(operation) = operation {
            extra.insert("canvasProgramOperation".to_string(), json!(operation));
        }
        payload.extra_body = Some(extra);
        payload
    }

    #[test]
    fn program_video_direct_http_attempt_requires_pure_http_or_concrete_handle() {
        let mut payload = make_payload("gemini_canvas_program_web_reverse_compatible");
        payload.extra_body = Some(HashMap::from([(
            "pureHttpMode".to_string(),
            Value::String("preferred".to_string()),
        )]));

        assert!(
            !should_attempt_gemini_canvas_program_modular_media_direct_http(
                &payload,
                gemini_canvas::GeminiCanvasMediaOperation::Video,
            )
        );

        let mut payload = make_gemini_canvas_program_payload_with_handle(None);
        payload.extra_body.as_mut().expect("extra").insert(
            "pureHttpMode".to_string(),
            Value::String("preferred".to_string()),
        );

        assert!(
            should_attempt_gemini_canvas_program_modular_media_direct_http(
                &payload,
                gemini_canvas::GeminiCanvasMediaOperation::Video,
            )
        );
    }

    #[test]
    fn program_music_direct_http_attempt_stays_enabled_in_preferred_mode() {
        let mut payload = make_payload("gemini_canvas_program_web_reverse_compatible");
        payload.extra_body = Some(HashMap::from([(
            "pureHttpMode".to_string(),
            Value::String("preferred".to_string()),
        )]));

        assert!(
            should_attempt_gemini_canvas_program_modular_media_direct_http(
                &payload,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
            )
        );
    }

    #[test]
    fn program_direct_http_authoritative_when_adapter_is_program_owned() {
        let payload = make_payload("gemini_canvas_program_web_reverse_compatible");
        assert!(
            should_treat_gemini_canvas_program_modular_media_direct_http_as_authoritative(&payload)
        );
    }

    #[test]
    fn program_owned_payload_detects_explicit_official_api_identity() {
        let mut payload = make_payload("gemini_canvas_program_web_reverse_compatible");
        payload.extra_body = Some(HashMap::from([(
            "googleApiKey".to_string(),
            Value::String("AIzaSyCanvasForbiddenKey".to_string()),
        )]));
        assert!(gemini_canvas_program_payload_has_explicit_official_api_identity(&payload));
        let error = ensure_gemini_canvas_program_payload_avoids_official_api_identity(&payload)
            .expect_err("official api identity should be forbidden");
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_official_api_key_forbidden")
        );
    }

    #[test]
    fn program_owned_payload_allows_empty_official_identity_fields() {
        let mut payload = make_payload("gemini_canvas_program_web_reverse_compatible");
        payload.extra_body = Some(HashMap::from([
            ("googleApiKey".to_string(), Value::String(String::new())),
            ("apiKeys".to_string(), Value::Array(Vec::new())),
        ]));
        assert!(!gemini_canvas_program_payload_has_explicit_official_api_identity(&payload));
        ensure_gemini_canvas_program_payload_avoids_official_api_identity(&payload)
            .expect("empty fields should not trigger forbidden official api identity");
    }

    #[test]
    fn program_runtime_material_official_api_identity_error_matches_contract() {
        let error = gemini_canvas_program_runtime_material_official_api_key_forbidden_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_program_web_reverse_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_official_api_key_forbidden")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas program-owned runtime material still carries official Google API keys. Remove googleApiKey/apiKeys from the materialized browser-state before invoking this line."
        );
    }

    #[test]
    fn program_tts_pure_http_required_error_matches_contract() {
        let error = gemini_canvas_program_tts_pure_http_required_error(
            "gemini_canvas_program_web_reverse_compatible",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_program_web_reverse_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_tts_pure_http_required")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas program-owned TTS lane requires the pure HTTP page-owned Canvas program contract."
        );
    }

    #[test]
    fn program_text_pure_http_required_error_matches_contract() {
        let error = gemini_canvas_program_text_pure_http_required_error(
            "gemini_canvas_program_web_reverse_compatible",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_program_web_reverse_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_text_pure_http_required")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas program-owned text lane requires the pure HTTP page-owned Canvas program contract."
        );
    }

    #[test]
    fn program_image_pure_http_required_error_matches_contract() {
        let error = gemini_canvas_program_image_pure_http_required_error(
            "gemini_canvas_program_web_reverse_compatible",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_program_web_reverse_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_image_pure_http_required")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas program-owned image lane requires the pure HTTP page-owned Canvas program contract."
        );
    }

    #[test]
    fn program_direct_http_authoritative_when_execution_mode_is_direct_http() {
        let mut payload = make_payload("gemini_canvas_program_web_reverse_compatible");
        payload.execution_mode = Some(ProviderExecutionMode::DirectHttp);
        assert!(
            should_treat_gemini_canvas_program_modular_media_direct_http_as_authoritative(&payload)
        );
    }

    #[test]
    fn program_direct_http_authoritative_when_pure_http_required() {
        let mut payload = make_payload("gemini_canvas_program_web_reverse_compatible");
        payload.extra_body = Some(HashMap::from([(
            "pureHttpMode".to_string(),
            Value::String("required".to_string()),
        )]));
        assert!(
            should_treat_gemini_canvas_program_modular_media_direct_http_as_authoritative(&payload)
        );
    }
}
