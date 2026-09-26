//! Candidate endpoint policy ownership.
use super::*;

pub(super) fn observe_provider_rate_limit_rejection(
    rejections: &mut ProviderRateLimitRejections,
    error: &GatewayError,
) -> bool {
    if error.code.as_deref() != Some("rate_limit_exceeded") {
        return false;
    }
    let FallbackHint::Retry { delay_ms, .. } = &error.fallback_hint else {
        return false;
    };
    rejections.observe(*delay_ms);
    true
}

pub(super) fn expects_json_passthrough(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
) -> bool {
    match req.endpoint_kind {
        EndpointKind::Embeddings
        | EndpointKind::ImagesGenerations
        | EndpointKind::ImagesEdits
        | EndpointKind::MusicGenerations
        | EndpointKind::VideosGenerations
        | EndpointKind::Search
        | EndpointKind::Fetch
        | EndpointKind::ResearchCreate
        | EndpointKind::ResearchList
        | EndpointKind::ResearchGet
        | EndpointKind::CreditsBalance => true,
        EndpointKind::AudioTranscriptions => !matches!(
            req.raw_body
                .get("response_format")
                .and_then(|value| value.as_str())
                .map(|value| value.to_ascii_lowercase()),
            Some(format) if matches!(format.as_str(), "text" | "srt" | "vtt")
        ),
        _ => false,
    }
}

pub(super) fn expects_binary_passthrough(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
) -> bool {
    matches!(req.endpoint_kind, EndpointKind::AudioSpeech)
        || (req.endpoint_kind == EndpointKind::AudioTranscriptions
            && !expects_json_passthrough(req))
}

pub(super) fn is_conversation_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Completions
            | EndpointKind::Messages
            | EndpointKind::Responses
    )
}
