use super::routing_diagnostics::parse_endpoint_kind;
use crate::protocol::canonical::EndpointKind;

#[test]
fn parse_endpoint_kind_supports_legacy_completions() {
    assert!(matches!(
        parse_endpoint_kind("completions"),
        Ok(EndpointKind::Completions)
    ));
}

#[test]
fn parse_endpoint_kind_supports_audio_speech() {
    assert!(matches!(
        parse_endpoint_kind("audio_speech"),
        Ok(EndpointKind::AudioSpeech)
    ));
}

#[test]
fn parse_endpoint_kind_supports_embeddings() {
    assert!(matches!(
        parse_endpoint_kind("embeddings"),
        Ok(EndpointKind::Embeddings)
    ));
}
