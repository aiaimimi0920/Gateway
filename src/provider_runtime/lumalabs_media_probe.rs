//! Explicit image-only Luma Web consent, without inventing a model/action binding.
use super::{
    media_probe_request::{MediaProbeOperation, MediaProbeRequest},
    ConsoleProbeRequest,
};
use crate::{
    error::GatewayError, protocol::canonical::CanonicalRelayRequest,
    routing::candidate::ProviderAccountPayload,
};

pub(super) fn canonical(
    media: &MediaProbeRequest,
    request: &ConsoleProbeRequest,
    model: &str,
    payload: &ProviderAccountPayload,
) -> Result<CanonicalRelayRequest, GatewayError> {
    #[cfg(feature = "line-lumalabs-web-reverse-api")]
    {
        use crate::{
            protocol::{call_flow::SpecialAdapter, canonical::EndpointKind},
            routing::candidate::ProviderExecutionMode,
        };
        if !matches!(media.operation, MediaProbeOperation::Image)
            || !media.parameters.is_empty()
            || !payload
                .call_flow
                .is_some_and(|flow| flow.special_adapter == SpecialAdapter::LumalabsWeb)
            || payload.base_url.trim_end_matches('/') != "https://app.lumalabs.ai"
            || payload.resolve_execution_mode(EndpointKind::ImagesGenerations)
                != ProviderExecutionMode::BrowserBacked
            || crate::protocol::call_flow::prepare(payload)?.executor != "lumalabs_compatible"
        {
            return Err(invalid());
        }
        // The ordinary image sender validates bytes even for URL output. Keep them out of
        // the probe response/audit and disallow request-level action/realm/count overrides.
        let canonical =
            crate::protocol::gemini_business::normalize_image_generations(serde_json::json!({
                "model":request.model,"prompt":request.prompt,"response_format":"url"
            }))?;
        crate::upstream::lumalabs::validate_media_probe(payload, &canonical, model)?;
        Ok(canonical)
    }
    #[cfg(not(feature = "line-lumalabs-web-reverse-api"))]
    {
        let _ = (media, request, model, payload);
        Err(invalid())
    }
}

fn invalid() -> GatewayError {
    GatewayError::bad_request("Luma Web tests require the typed browser executor, fixed audience, image operation and no parameter overrides.")
        .with_provider("lumalabs_compatible").with_code("lumalabs_probe_input_invalid")
}
