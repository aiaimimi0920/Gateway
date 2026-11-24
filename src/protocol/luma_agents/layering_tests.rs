//! Layer extraction must not borrow image creation/edit controls or their prompt/model defaults.
use super::*;
use serde_json::{json, Value};
fn body() -> Value {
    json!({"model":"alias","type":"layering","prompt":"", "source":{"url":"https://images.example.test/source.png"}})
}
fn pack(value: Value) -> Result<GenerationRequest, GatewayError> {
    GenerationRequest::pack(&normalize_layering(value)?, "uni-1")
}
#[test]
fn luma_layering_accepts_empty_guidance_four_sources_and_resolved_uni1() {
    for source in [
        body()["source"].clone(),
        json!({"data":"YQ==","media_type":"image/png"}),
        json!({"file_id":"d290f1ee-6c54-4b01-90e6-d701748f0851"}),
        json!({"generation_id":"d290f1ee-6c54-4b01-90e6-d701748f0851"}),
    ] {
        let mut value = body();
        value["source"] = source.clone();
        let mapped = pack(value).unwrap();
        assert_eq!(mapped.media, Media::Image);
        assert_eq!(mapped.kind, "layering");
        assert_eq!(
            mapped.body,
            json!({"model":"uni-1","type":"layering","prompt":"","source":source})
        );
    }
}
#[test]
fn luma_layering_preserves_short_unicode_guidance_and_resolution() {
    for resolution in ["1k", "2k"] {
        let mut value = body();
        value["prompt"] = json!("分".repeat(500));
        value["layering"] = json!({"resolution":resolution});
        let mapped = pack(value.clone()).unwrap();
        assert_eq!(mapped.body["prompt"], value["prompt"]);
        assert_eq!(mapped.body["layering"], value["layering"]);
    }
    for prompt in [Value::Null, json!(false), json!("x".repeat(501))] {
        let mut value = body();
        value["prompt"] = prompt;
        assert!(pack(value).is_err());
    }
    let mut missing = body();
    missing.as_object_mut().unwrap().remove("prompt");
    assert!(pack(missing).is_err());
    assert!(
        crate::protocol::gemini_business::normalize_image_generations(json!({"prompt":""}))
            .is_err()
    );
}
#[test]
fn luma_layering_rejects_foreign_models_endpoints_and_cross_operation_controls() {
    let mut canonical = normalize_layering(body()).unwrap();
    for model in ["uni-1-max", "ray-3.2"] {
        assert!(GenerationRequest::pack(&canonical, model).is_err());
    }
    canonical.endpoint_kind = crate::protocol::canonical::EndpointKind::ImagesEdits;
    assert!(GenerationRequest::pack(&canonical, "uni-1").is_err());
    for change in [
        json!({"source":null}),
        json!({"aspect_ratio":"1:1"}),
        json!({"style":"auto"}),
        json!({"output_format":"jpeg"}),
        json!({"image_ref":[]}),
        json!({"web_search":false}),
        json!({"video":{}}),
        json!({"images":[]}),
        json!({"layering":{"resolution":"4k"}}),
        json!({"layering":{"count":2}}),
        json!({"layering":[]}),
        json!({"n":2}),
    ] {
        let mut value = body();
        value
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(pack(value).is_err(), "{change}");
    }
    let normal = crate::protocol::gemini_business::normalize_image_generations(
        json!({"model":"uni-1","prompt":"create","layering":{"resolution":"1k"}}),
    )
    .unwrap();
    assert!(GenerationRequest::pack(&normal, "uni-1").is_err());
}
#[test]
fn luma_layering_empty_probe_guidance_never_relaxes_other_media_operations() {
    use crate::provider_runtime::ConsoleProbeRequest;
    for operation in [
        "layering",
        "image",
        "image_edit",
        "video",
        "video_edit",
        "video_reframe",
        "music",
    ] {
        let input = json!({"prompt":"","model":"uni-1","credentialIds":["exact"],
            "media":{"operation":operation,"confirmCharge":true,"parameters":{}}});
        assert_eq!(
            ConsoleProbeRequest::parse(input.to_string().as_bytes()).is_ok(),
            operation == "layering"
        );
    }
}
