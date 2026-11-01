//! The new music-only template must not inherit Producer's browser video surface.
use super::*;
use crate::protocol::canonical::EndpointKind;

#[test]
fn flowmusic_template_compiles_as_http_music_only_with_empty_or_explicit_credentials() {
    let store = make_store_from_yaml(
        r#"
providers:
  - id: flowmusic-web
    preset: producer
    base_url: https://www.flowmusic.app
    explicit_credentials_only: true
    supported_models: ["producer:standard"]
    supported_protocol_families: [producer_music]
    call_flow: {call_adapter: auto, fingerprint_adapter: session, special_adapter: flowmusic_web}
    credentials:
      - id: selected
        runtime_state_object_key: credential-runtime/flowmusic/fixture/http-session.json
model_routes:
  - pattern: "producer:standard"
    provider_ids: [flowmusic-web]
"#,
    );
    let candidates = store.resolve_candidates(Some("producer:standard"));
    assert_eq!(candidates.len(), 1);
    let candidate = &candidates[0];
    assert_eq!(candidate.adapter, "producer_compatible");
    assert_eq!(candidate.supported_protocol_families, ["producer_music"]);
    assert_eq!(
        candidate
            .payload
            .resolve_execution_mode(EndpointKind::MusicGenerations),
        crate::routing::candidate::ProviderExecutionMode::DirectHttp
    );
    assert_eq!(
        candidate
            .payload
            .resolve_execution_mode(EndpointKind::VideosGenerations),
        crate::routing::candidate::ProviderExecutionMode::DirectHttp
    );
}
