use crate::descriptor::{
    find_by_preset, EndpointSurface, ProviderAuthKind, ProviderDescriptor, ProviderUiFieldSpec,
};
use crate::sse::{BoundedSseDecoder, SseEventFrame};

const SAMPLE_FIELDS: &[ProviderUiFieldSpec] = &[ProviderUiFieldSpec {
    key: "api_key",
    label_zh: "API Key",
    label_en: "API Key",
    secret: true,
    required: true,
    placeholder: "sk-...",
}];

const SAMPLE_PROVIDER: ProviderDescriptor = ProviderDescriptor {
    line_feature: "line-sample-official-api",
    preset_id: "sample-api",
    vendor_key: "sample",
    vendor_name: "Sample AI",
    default_base_url: "https://api.sample.ai/v1",
    allowed_base_urls: &["https://api.sample.ai/v1"],
    models_catalog_path: Some("/models"),
    auth_kind: ProviderAuthKind::BearerApiKey,
    endpoints: &[EndpointSurface::ChatCompletions, EndpointSurface::Responses],
    ui_fields: SAMPLE_FIELDS,
};

#[test]
fn validates_exact_https_origin_and_rejects_spoofed_or_unsafe_urls() {
    assert_eq!(
        SAMPLE_PROVIDER
            .validate_base_url("https://api.sample.ai/v1/")
            .expect("valid origin"),
        "https://api.sample.ai/v1"
    );
    assert_eq!(
        SAMPLE_PROVIDER
            .resolve_models_url("https://api.sample.ai/v1")
            .expect("models url"),
        Some("https://api.sample.ai/v1/models".to_string())
    );
    for invalid in [
        "",
        "http://api.sample.ai/v1",
        "https://user:pass@api.sample.ai/v1",
        "https://api.sample.ai/v1?api_key=leak",
        "https://api.sample.ai/v1#frag",
        "https://api.sample.ai.evil.invalid/v1",
        "https://console.sample.ai/v1",
    ] {
        assert!(
            SAMPLE_PROVIDER.validate_base_url(invalid).is_err(),
            "expected rejection for {invalid}"
        );
    }
}

#[test]
fn registry_lookup_and_endpoint_capabilities_match_declaration() {
    let registry = [SAMPLE_PROVIDER];
    let found = find_by_preset(&registry, "sample-api").expect("find sample-api");
    assert!(found.supports_endpoint(EndpointSurface::ChatCompletions));
    assert!(!found.supports_endpoint(EndpointSurface::AnthropicMessages));
    assert!(find_by_preset(&registry, "unknown").is_none());
}

#[test]
fn sse_decoder_handles_split_chunks_crlf_comments_and_overflow_bounds() {
    let mut decoder = BoundedSseDecoder::new(1024);
    assert!(decoder
        .push_chunk(b": keepalive\r\nevent: message_start\r\nda")
        .unwrap()
        .is_empty());
    let frames = decoder
        .push_chunk(b"ta: {\"ok\":true}\r\n\r\ndata: [DONE]\n\n")
        .expect("decode frames");
    assert_eq!(
        frames,
        vec![
            SseEventFrame {
                event: Some("message_start".to_string()),
                data: "{\"ok\":true}".to_string(),
            },
            SseEventFrame {
                event: None,
                data: "[DONE]".to_string(),
            },
        ]
    );

    let mut tiny = BoundedSseDecoder::new(1024);
    let oversized = vec![b'a'; 2048];
    assert!(tiny.push_chunk(&oversized).is_err());

    let mut coalesced = BoundedSseDecoder::new(1024);
    let mut batch = Vec::new();
    for i in 0..80 {
        let delim = match i % 3 {
            0 => "\n\n",
            1 => "\n\r\n",
            _ => "\r\n\n",
        };
        batch.extend_from_slice(format!("data: {{\"n\":{i}}}{delim}").as_bytes());
    }
    assert!(batch.len() > 1024);
    let batch_frames = coalesced
        .push_chunk(&batch)
        .expect("coalesced multi-frame chunk within per-frame bound");
    assert_eq!(batch_frames.len(), 80);
    assert_eq!(batch_frames[0].data, "{\"n\":0}");
    assert_eq!(batch_frames[79].data, "{\"n\":79}");
}
