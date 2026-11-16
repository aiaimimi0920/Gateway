use super::*;
use crate::protocol::canonical::EndpointKind;
use serde_json::{json, Value};

fn pack(
    body: Value,
    endpoint: EndpointKind,
    model: &str,
) -> Result<GenerationRequest, crate::error::GatewayError> {
    let mut request = crate::protocol::gemini_business::normalize_image_generations(body)?;
    request.endpoint_kind = endpoint;
    GenerationRequest::pack(&request, model)
}

#[test]
fn luma_agents_request_preserves_unicode_and_uses_resolved_model_not_caller_alias() {
    let result = pack(
        json!({"model":"alias","prompt":"中文 &amp; image","aspect_ratio":"2:3","style":"manga",
        "output_format":"jpeg","web_search":false,"user_id":"opaque-1"}),
        EndpointKind::ImagesGenerations,
        "uni-1-max",
    )
    .unwrap();
    assert_eq!(
        result.body,
        json!({"model":"uni-1-max","type":"image","prompt":"中文 &amp; image",
        "aspect_ratio":"2:3","style":"manga","output_format":"jpeg","web_search":false,"user_id":"opaque-1"})
    );
}
#[test]
fn luma_agents_request_rejects_unsupported_controls_and_bad_types() {
    for extra in [
        json!({"n":0}),
        json!({"n":2}),
        json!({"n":true}),
        json!({"stream":true}),
        json!({"web_search":"true"}),
        json!({"aspect_ratio":"auto"}),
        json!({"size":"1024x1024"}),
        json!({"style":"manga","aspect_ratio":"1:1"}),
        json!({"source":{"url":"https://example.com/a.png"}}),
        json!({"response_format":"jpeg"}),
        json!({"video":{}}),
        json!({"callback_url":"https://example.com"}),
        json!({"prompt":"x".repeat(6001)}),
        json!({"type":"image_edit"}),
    ] {
        let mut body = json!({"model":"uni-1","prompt":"image"});
        body.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert!(
            pack(body.clone(), EndpointKind::ImagesGenerations, "uni-1").is_err(),
            "{body}"
        );
    }
    assert!(pack(
        json!({"model":"ray-3.2","prompt":"video"}),
        EndpointKind::ImagesGenerations,
        "ray-3.2"
    )
    .is_err());
    assert!(pack(
        json!({"model":"uni-1","prompt":"chat"}),
        EndpointKind::ChatCompletions,
        "uni-1"
    )
    .is_err());
}
#[test]
fn luma_agents_image_edit_source_and_reference_limits_keep_distinct_meanings() {
    for source in [
        json!({"url":"https://example.com/a.png"}),
        json!({"data":"YQ==","media_type":"image/png"}),
        json!({"file_id":"d290f1ee-6c54-4b01-90e6-d701748f0851"}),
        json!({"generation_id":"d290f1ee-6c54-4b01-90e6-d701748f0851"}),
    ] {
        let result = pack(
            json!({"model":"uni-1","prompt":"edit","source":source,"style":"manga"}),
            EndpointKind::ImagesEdits,
            "uni-1",
        )
        .unwrap();
        assert_eq!(result.body["type"], "image_edit");
        assert_eq!(result.body["source"], source);
    }
    let nine = vec![json!({"url":"https://example.com/ref.png"}); 9];
    assert!(pack(
        json!({"prompt":"image","image_ref":nine}),
        EndpointKind::ImagesGenerations,
        "uni-1"
    )
    .is_ok());
    assert!(pack(
        json!({"prompt":"edit","source":{"url":"https://example.com/a.png"},"image_ref":nine}),
        EndpointKind::ImagesEdits,
        "uni-1"
    )
    .is_err());
    for source in [
        json!({"url":"https://example.com","data":"YQ==","media_type":"image/png"}),
        json!({"data":"not base64","media_type":"image/png"}),
        json!({"data":"YQ=="}),
        json!({"url":"http://example.com"}),
        json!({"file_id":"../other"}),
    ] {
        assert!(pack(
            json!({"prompt":"edit","source":source}),
            EndpointKind::ImagesEdits,
            "uni-1"
        )
        .is_err());
    }
}
#[test]
fn luma_agents_multipart_upload_projection_and_total_inline_budget_are_bounded() {
    let result=pack(json!({"prompt":"edit","images":[{"data_url":"data:image/png;base64,YQ==","mime_type":"image/png","filename":"not-a-path.png"}]}),EndpointKind::ImagesEdits,"uni-1").unwrap();
    assert_eq!(
        result.body["source"],
        json!({"data":"YQ==","media_type":"image/png"})
    );
    let large = json!({"data":"YQ==".repeat(2*1024*1024),"media_type":"image/png"});
    assert!(pack(
        json!({"prompt":"image","image_ref":[large.clone(),large.clone(),large]}),
        EndpointKind::ImagesGenerations,
        "uni-1"
    )
    .is_err());
}
#[test]
fn luma_agents_video_controls_preserve_keyframes_and_validate_combinations() {
    let good = json!({"prompt":"video","aspect_ratio":"21:9","video":{"resolution":"1080p","duration":"5s",
        "keyframes":[{"url":"https://example.com/a.png"},{"url":"https://example.com/b.png"}],"keyframe_indexes":[0,120]}});
    assert_eq!(
        pack(good.clone(), EndpointKind::VideosGenerations, "ray-3.2")
            .unwrap()
            .body["video"],
        good["video"]
    );
    for video in [
        json!({"resolution":"360p","hdr":true}),
        json!({"duration":"10s","hdr":true}),
        json!({"duration":"10s","loop":true}),
        json!({"hdr":true,"loop":true}),
        json!({"exr_export":true}),
        json!({"duration":"10s","start_frame":{"url":"https://example.com/a.png"}}),
        json!({"loop":true,"end_frame":{"url":"https://example.com/a.png"}}),
        json!({"keyframes":[{"url":"https://example.com/a.png"}],"keyframe_indexes":[121]}),
        json!({"keyframes":[{"url":"https://example.com/a.png"},{"url":"https://example.com/b.png"}],"keyframe_indexes":[0,0]}),
        json!({"keyframes":[{"url":"https://example.com/a.png"}]}),
        json!({"edit":{}}),
    ] {
        assert!(
            pack(
                json!({"prompt":"video","video":video}),
                EndpointKind::VideosGenerations,
                "ray-3.2"
            )
            .is_err(),
            "{video}"
        );
    }
}
#[test]
fn luma_agents_identity_cannot_use_web_or_generic_executor() {
    let mut payload: crate::routing::candidate::ProviderAccountPayload = serde_json::from_value(
        json!({"adapter":ADAPTER,"base_url":BASE,"api_key":"luma-api-synthetic"}),
    )
    .unwrap();
    assert!(crate::protocol::call_flow::prepare(&payload).is_ok());
    assert!(validate(&payload, "openai_compatible").is_err());
    for base in [
        "https://app.lumalabs.ai",
        "https://api.lumalabs.ai/dream-machine/v1",
        "https://agents.lumalabs.ai/v1?key=x",
        "http://agents.lumalabs.ai/v1",
    ] {
        payload.base_url = base.into();
        assert!(crate::protocol::call_flow::prepare(&payload).is_err());
    }
    payload.base_url = BASE.into();
    payload.adapter = "openai_compatible".into();
    assert!(crate::protocol::call_flow::prepare(&payload).is_err());
    payload.base_url = "https://different-product.example/v1".into();
    assert!(crate::protocol::call_flow::prepare(&payload).is_err());
}

#[test]
fn luma_agents_multi_keyframes_accept_prior_generation_images() {
    let body = json!({"prompt":"video","video":{"duration":"10s","keyframes":[
        {"generation_id":"d290f1ee-6c54-4b01-90e6-d701748f0851"}],"keyframe_indexes":[240]}});
    assert_eq!(
        pack(body.clone(), EndpointKind::VideosGenerations, "ray-3.2")
            .unwrap()
            .body["video"],
        body["video"]
    );
}
