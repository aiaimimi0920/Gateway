use std::collections::HashMap;

use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::gemini::web_reverse as surface;
use crate::protocol::gemini_canvas::GeminiCanvasMediaOperation;
use crate::routing::candidate::{ProviderAccountPayload, ProviderExecutionMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyMixedLaneEndpointClass {
    Text,
    Tts,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyMixedLaneExecutionKind {
    TextAccumulate,
    TextStream,
    Tts,
    Media,
}

#[derive(Debug, Clone)]
pub struct LegacyMixedLaneExecutionRoute {
    pub kind: LegacyMixedLaneExecutionKind,
    pub payload: ProviderAccountPayload,
}

pub fn prompt_for_legacy_mixed_lane_media_request(
    req: &CanonicalRelayRequest,
    operation: GeminiCanvasMediaOperation,
) -> Result<String, GatewayError> {
    surface::prompt_for_legacy_mixed_lane_media_request(req, operation)
}

pub fn is_legacy_mixed_lane_adapter(payload: &ProviderAccountPayload) -> bool {
    payload.adapter == surface::GEMINI_WEB_REVERSE_MODULAR_ADAPTER
}

pub fn classify_legacy_mixed_lane_endpoint(
    endpoint_kind: EndpointKind,
) -> LegacyMixedLaneEndpointClass {
    if supports_legacy_mixed_lane_text_endpoint(endpoint_kind) {
        LegacyMixedLaneEndpointClass::Text
    } else if supports_legacy_mixed_lane_tts_endpoint(endpoint_kind) {
        LegacyMixedLaneEndpointClass::Tts
    } else {
        LegacyMixedLaneEndpointClass::Other
    }
}

pub fn force_legacy_mixed_lane_payload(payload: &ProviderAccountPayload) -> ProviderAccountPayload {
    let mut cloned = payload.clone();
    cloned.execution_mode = Some(ProviderExecutionMode::DirectHttp);
    if let Some(extra_body) = cloned.extra_body.as_mut() {
        extra_body.insert(
            "pureHttpMode".to_string(),
            Value::String("enabled".to_string()),
        );
        extra_body.remove("canvasExecutionOwner");
    } else {
        let mut extra_body = HashMap::new();
        extra_body.insert(
            "pureHttpMode".to_string(),
            Value::String("enabled".to_string()),
        );
        cloned.extra_body = Some(extra_body);
    }
    cloned
}

pub fn legacy_mixed_lane_payload_for_endpoint(
    payload: &ProviderAccountPayload,
    endpoint_kind: EndpointKind,
) -> Option<ProviderAccountPayload> {
    legacy_mixed_lane_execution_route(payload, endpoint_kind, false).and_then(|route| {
        matches!(
            route.kind,
            LegacyMixedLaneExecutionKind::TextAccumulate | LegacyMixedLaneExecutionKind::Tts
        )
        .then_some(route.payload)
    })
}

pub fn legacy_mixed_lane_text_payload(
    payload: &ProviderAccountPayload,
    endpoint_kind: EndpointKind,
) -> Option<ProviderAccountPayload> {
    legacy_mixed_lane_execution_route(payload, endpoint_kind, false).and_then(|route| {
        (route.kind == LegacyMixedLaneExecutionKind::TextAccumulate).then_some(route.payload)
    })
}

pub fn legacy_mixed_lane_tts_payload(
    payload: &ProviderAccountPayload,
    endpoint_kind: EndpointKind,
) -> Option<ProviderAccountPayload> {
    legacy_mixed_lane_execution_route(payload, endpoint_kind, false).and_then(|route| {
        (route.kind == LegacyMixedLaneExecutionKind::Tts).then_some(route.payload)
    })
}

pub fn legacy_mixed_lane_media_payload(
    payload: &ProviderAccountPayload,
    endpoint_kind: EndpointKind,
) -> Option<ProviderAccountPayload> {
    legacy_mixed_lane_execution_route(payload, endpoint_kind, false).and_then(|route| {
        (route.kind == LegacyMixedLaneExecutionKind::Media).then_some(route.payload)
    })
}

pub fn supports_legacy_mixed_lane_text_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions
    )
}

pub fn supports_legacy_mixed_lane_tts_endpoint(endpoint_kind: EndpointKind) -> bool {
    endpoint_kind == EndpointKind::AudioSpeech
}

pub fn supports_legacy_mixed_lane_media_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ImagesGenerations
            | EndpointKind::ImagesEdits
            | EndpointKind::MusicGenerations
            | EndpointKind::VideosGenerations
    )
}

pub fn legacy_mixed_lane_execution_route(
    payload: &ProviderAccountPayload,
    endpoint_kind: EndpointKind,
    wants_stream: bool,
) -> Option<LegacyMixedLaneExecutionRoute> {
    if !is_legacy_mixed_lane_adapter(payload) {
        return None;
    }

    let kind = if supports_legacy_mixed_lane_text_endpoint(endpoint_kind) {
        if wants_stream {
            LegacyMixedLaneExecutionKind::TextStream
        } else {
            LegacyMixedLaneExecutionKind::TextAccumulate
        }
    } else if supports_legacy_mixed_lane_tts_endpoint(endpoint_kind) {
        LegacyMixedLaneExecutionKind::Tts
    } else if supports_legacy_mixed_lane_media_endpoint(endpoint_kind) {
        LegacyMixedLaneExecutionKind::Media
    } else {
        return None;
    };

    Some(LegacyMixedLaneExecutionRoute {
        kind,
        payload: force_legacy_mixed_lane_payload(payload),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_payload(adapter: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: adapter.to_string(),
            base_url: "https://gemini.google.com".to_string(),
            api_key: "psid-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    #[test]
    fn detects_legacy_mixed_lane_adapter_only_for_modular_web_reverse() {
        assert!(is_legacy_mixed_lane_adapter(&make_payload(
            surface::GEMINI_WEB_REVERSE_MODULAR_ADAPTER,
        )));
        assert!(!is_legacy_mixed_lane_adapter(&make_payload(
            "gemini_canvas_compatible",
        )));
    }

    #[test]
    fn classifies_text_tts_and_other_endpoints() {
        assert_eq!(
            classify_legacy_mixed_lane_endpoint(EndpointKind::ChatCompletions),
            LegacyMixedLaneEndpointClass::Text
        );
        assert_eq!(
            classify_legacy_mixed_lane_endpoint(EndpointKind::Responses),
            LegacyMixedLaneEndpointClass::Text
        );
        assert_eq!(
            classify_legacy_mixed_lane_endpoint(EndpointKind::AudioSpeech),
            LegacyMixedLaneEndpointClass::Tts
        );
        assert_eq!(
            classify_legacy_mixed_lane_endpoint(EndpointKind::ImagesGenerations),
            LegacyMixedLaneEndpointClass::Other
        );
    }

    #[test]
    fn payload_for_endpoint_only_coerces_text_and_tts_for_modular_adapter() {
        let payload = make_payload(surface::GEMINI_WEB_REVERSE_MODULAR_ADAPTER);
        let text_payload = legacy_mixed_lane_payload_for_endpoint(&payload, EndpointKind::Messages)
            .expect("text payload");
        assert_eq!(
            text_payload.adapter,
            surface::GEMINI_WEB_REVERSE_MODULAR_ADAPTER
        );
        assert_eq!(
            text_payload.execution_mode,
            Some(ProviderExecutionMode::DirectHttp)
        );
        assert_eq!(
            text_payload
                .extra_body
                .as_ref()
                .and_then(|body| body.get("pureHttpMode"))
                .and_then(Value::as_str),
            Some("enabled")
        );

        let tts_payload =
            legacy_mixed_lane_payload_for_endpoint(&payload, EndpointKind::AudioSpeech)
                .expect("tts payload");
        assert_eq!(
            tts_payload.adapter,
            surface::GEMINI_WEB_REVERSE_MODULAR_ADAPTER
        );

        assert!(
            legacy_mixed_lane_payload_for_endpoint(&payload, EndpointKind::ImagesGenerations)
                .is_none()
        );
        assert!(legacy_mixed_lane_payload_for_endpoint(
            &make_payload("gemini_canvas_compatible"),
            EndpointKind::Messages
        )
        .is_none());
    }

    #[test]
    fn specialized_payload_helpers_follow_classified_endpoint_roles() {
        let payload = make_payload(surface::GEMINI_WEB_REVERSE_MODULAR_ADAPTER);
        assert!(legacy_mixed_lane_text_payload(&payload, EndpointKind::Completions).is_some());
        assert!(legacy_mixed_lane_text_payload(&payload, EndpointKind::AudioSpeech).is_none());
        assert!(legacy_mixed_lane_tts_payload(&payload, EndpointKind::AudioSpeech).is_some());
        assert!(legacy_mixed_lane_tts_payload(&payload, EndpointKind::Responses).is_none());
        assert!(
            legacy_mixed_lane_media_payload(&payload, EndpointKind::ImagesGenerations).is_some()
        );
    }

    #[test]
    fn execution_route_splits_text_stream_tts_and_media_roles() {
        let payload = make_payload(surface::GEMINI_WEB_REVERSE_MODULAR_ADAPTER);
        let text =
            legacy_mixed_lane_execution_route(&payload, EndpointKind::ChatCompletions, false)
                .expect("text route");
        assert_eq!(text.kind, LegacyMixedLaneExecutionKind::TextAccumulate);

        let stream =
            legacy_mixed_lane_execution_route(&payload, EndpointKind::ChatCompletions, true)
                .expect("stream route");
        assert_eq!(stream.kind, LegacyMixedLaneExecutionKind::TextStream);

        let tts = legacy_mixed_lane_execution_route(&payload, EndpointKind::AudioSpeech, false)
            .expect("tts route");
        assert_eq!(tts.kind, LegacyMixedLaneExecutionKind::Tts);

        let media =
            legacy_mixed_lane_execution_route(&payload, EndpointKind::VideosGenerations, false)
                .expect("media route");
        assert_eq!(media.kind, LegacyMixedLaneExecutionKind::Media);

        assert!(legacy_mixed_lane_execution_route(
            &make_payload("gemini_canvas_compatible"),
            EndpointKind::ChatCompletions,
            false,
        )
        .is_none());
    }
}
