use std::collections::HashMap;
use std::time::Duration;

use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::aistudio_web as surface;
use crate::protocol::canonical::EndpointKind;
use crate::routing::candidate::{ProviderAccountPayload, ProviderExecutionMode};

pub fn is_aistudio_web_reverse_adapter(adapter: &str) -> bool {
    adapter == surface::AISTUDIO_WEB_REVERSE_ADAPTER
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonPassthroughRoute {
    Embeddings,
    ImagesGenerations,
}

pub fn json_passthrough_route(
    endpoint_kind: EndpointKind,
) -> Result<JsonPassthroughRoute, GatewayError> {
    match endpoint_kind {
        EndpointKind::Embeddings => Ok(JsonPassthroughRoute::Embeddings),
        EndpointKind::ImagesGenerations => Ok(JsonPassthroughRoute::ImagesGenerations),
        _ => Err(unsupported_json_passthrough_endpoint_error()),
    }
}

pub fn supports_binary_passthrough_endpoint(endpoint_kind: EndpointKind) -> bool {
    endpoint_kind == EndpointKind::AudioSpeech
}

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "AI Studio Web reverse adapters use a custom Gemini-native send path and are not supported by the generic request planner.",
    )
    .with_code("unsupported_aistudio_web_reverse_request_plan")
}

pub fn unsupported_json_passthrough_endpoint_error() -> GatewayError {
    GatewayError::bad_request(
        "AI Studio Web reverse JSON passthrough currently supports text, /v1/embeddings, and /v1/images/generations.",
    )
    .with_provider(surface::AISTUDIO_WEB_REVERSE_ADAPTER)
    .with_code("unsupported_aistudio_web_reverse_json_endpoint")
}

pub fn missing_aistudio_runtime_state_object_key_error() -> GatewayError {
    GatewayError::bad_request(
        "AI Studio Web reverse browser execution requires runtimeStateObjectKey.",
    )
    .with_code("missing_aistudio_runtime_state_object_key")
}

pub fn config_from_payload(
    payload: &ProviderAccountPayload,
) -> Result<surface::AIStudioWebConfig, GatewayError> {
    surface::config_from_payload(payload)
}

pub fn force_browser_owned_payload(payload: &ProviderAccountPayload) -> ProviderAccountPayload {
    let mut cloned = payload.clone();
    cloned.execution_mode = Some(ProviderExecutionMode::BrowserBacked);
    let extra_body = cloned.extra_body.get_or_insert_with(HashMap::new);
    extra_body.insert(
        "executionOwner".to_string(),
        Value::String("browser_owned_relay".to_string()),
    );
    cloned
}

pub fn build_browser_executor_invocation_input(
    payload: &ProviderAccountPayload,
    request_spec: &surface::AIStudioBrowserRequestSpec,
    timeout: Duration,
) -> Result<Value, GatewayError> {
    let config = config_from_payload(payload)?;
    let runtime_state_object_key = config
        .runtime_state_object_key
        .ok_or_else(missing_aistudio_runtime_state_object_key_error)?;

    Ok(json!({
        "runtimeStateObjectKey": runtime_state_object_key,
        "appUrl": config.app_url,
        "requestSpec": request_spec,
        "timeoutMs": timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        "browserExecutablePath": config.browser_executable_path,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::collections::HashMap;

    #[test]
    fn aistudio_web_reverse_adapter_predicate_accepts_only_surface_adapter() {
        assert!(is_aistudio_web_reverse_adapter(
            surface::AISTUDIO_WEB_REVERSE_ADAPTER
        ));
        assert!(!is_aistudio_web_reverse_adapter("gemini_web_compatible"));
        assert!(!is_aistudio_web_reverse_adapter(""));
    }

    #[test]
    fn json_passthrough_route_accepts_only_supported_non_text_endpoints() {
        assert_eq!(
            json_passthrough_route(crate::protocol::canonical::EndpointKind::Embeddings)
                .expect("embeddings should route"),
            JsonPassthroughRoute::Embeddings
        );
        assert_eq!(
            json_passthrough_route(crate::protocol::canonical::EndpointKind::ImagesGenerations)
                .expect("image generations should route"),
            JsonPassthroughRoute::ImagesGenerations
        );

        let err = json_passthrough_route(crate::protocol::canonical::EndpointKind::ImagesEdits)
            .expect_err("unsupported endpoint should fail fast");
        assert_eq!(
            err.code.as_deref(),
            Some("unsupported_aistudio_web_reverse_json_endpoint")
        );
        assert_eq!(
            err.provider_name.as_deref(),
            Some(surface::AISTUDIO_WEB_REVERSE_ADAPTER)
        );
    }

    #[test]
    fn supports_binary_passthrough_endpoint_accepts_only_audio_speech() {
        assert!(supports_binary_passthrough_endpoint(
            crate::protocol::canonical::EndpointKind::AudioSpeech
        ));
        assert!(!supports_binary_passthrough_endpoint(
            crate::protocol::canonical::EndpointKind::Embeddings
        ));
        assert!(!supports_binary_passthrough_endpoint(
            crate::protocol::canonical::EndpointKind::ImagesGenerations
        ));
    }

    #[test]
    fn missing_runtime_state_object_key_error_matches_contract() {
        let err = missing_aistudio_runtime_state_object_key_error();
        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.code.as_deref(),
            Some("missing_aistudio_runtime_state_object_key")
        );
        assert_eq!(
            err.message.as_str(),
            "AI Studio Web reverse browser execution requires runtimeStateObjectKey."
        );
    }

    #[test]
    fn unsupported_json_passthrough_endpoint_error_matches_contract() {
        let err = unsupported_json_passthrough_endpoint_error();
        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.provider_name.as_deref(),
            Some("aistudio_web_reverse_compatible")
        );
        assert_eq!(
            err.code.as_deref(),
            Some("unsupported_aistudio_web_reverse_json_endpoint")
        );
    }

    fn make_payload() -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: surface::AISTUDIO_WEB_REVERSE_ADAPTER.to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            api_key: "AIzaSyFixtureCloudApiKey000000000000000000".to_string(),
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
            extra_body: Some(HashMap::from([(
                "appUrl".to_string(),
                Value::String(surface::AISTUDIO_DEFAULT_APP_URL.to_string()),
            )])),
            session_auth: None,
            keepalive: None,
        }
    }

    #[test]
    fn browser_executor_input_still_requires_runtime_state() {
        let payload = make_payload();
        let request_spec = surface::build_browser_request_spec(
            "POST",
            "https://example.invalid/request".to_string(),
            HashMap::new(),
            Some(json!({"hello":"world"}).to_string()),
        );
        let error = build_browser_executor_invocation_input(
            &payload,
            &request_spec,
            Duration::from_secs(30),
        )
        .expect_err("missing runtime state should fail browser execution input");
        assert_eq!(
            error.code.as_deref(),
            Some("missing_aistudio_runtime_state_object_key")
        );
    }
}
