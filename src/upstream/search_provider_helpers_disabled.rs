use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

pub(crate) fn require_search_api_path<'a>(
    _payload: &'a ProviderAccountPayload,
    _endpoint_kind: EndpointKind,
) -> Result<&'a str, GatewayError> {
    Err(
        crate::implementation_lines::search_api_family_compiled_out_error(
            "search provider path resolution requested",
        ),
    )
}

pub(crate) fn search_endpoint_label(endpoint_kind: EndpointKind) -> &'static str {
    match endpoint_kind {
        EndpointKind::Search => "search",
        EndpointKind::Fetch => "fetch",
        EndpointKind::Completions => "completions",
        EndpointKind::Embeddings => "embeddings",
        EndpointKind::ImagesGenerations => "image generations",
        EndpointKind::ImagesEdits => "image edits",
        EndpointKind::MusicGenerations => "music generations",
        EndpointKind::VideosGenerations => "video generations",
        EndpointKind::AudioTranscriptions => "audio transcriptions",
        EndpointKind::AudioSpeech => "audio speech",
        EndpointKind::ResearchCreate | EndpointKind::ResearchList | EndpointKind::ResearchGet => {
            "research"
        }
        EndpointKind::CreditsBalance => "credits balance",
        EndpointKind::ChatCompletions => "chat completions",
        EndpointKind::Messages => "messages",
        EndpointKind::Responses => "responses",
    }
}

pub(crate) fn build_search_provider_request_plan(
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
) -> Result<RequestPlan, GatewayError> {
    Err(
        crate::implementation_lines::search_api_family_compiled_out_error(
            "search provider request plan requested",
        ),
    )
}
