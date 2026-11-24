//! Explicit edit contracts are distinct from create/reframe and reuse bounded media references.
use super::*;
use serde_json::{json, Value};

fn body() -> Value {
    json!({"model":"alias", "prompt":"重绘为胶片风格", "type":"video_edit",
        "source":{"generation_id":"d290f1ee-6c54-4b01-90e6-d701748f0851"}})
}
fn pack(value: Value) -> Result<GenerationRequest, GatewayError> {
    GenerationRequest::pack(
        &crate::protocol::gemini_canvas::normalize_video_generations(value)?,
        "ray-3.2",
    )
}
fn with_video(video: Value) -> Value {
    let mut value = body();
    value["video"] = video;
    value
}

#[test]
fn luma_video_edit_accepts_distinct_sources_and_resolved_model() {
    for source in [
        body()["source"].clone(),
        json!({"file_id":"d290f1ee-6c54-4b01-90e6-d701748f0851"}),
        json!({"url":"https://video.example.test/input.mp4","media_type":"video/mp4"}),
        json!({"data":"YQ==","media_type":"video/webm"}),
    ] {
        let mut value = body();
        value["source"] = source.clone();
        let request = pack(value).unwrap();
        assert_eq!(request.body["source"], source);
        assert_eq!(request.body["model"], "ray-3.2");
        assert_eq!(request.kind, "video_edit");
        assert_eq!(request.media, Media::Video);
    }
    for source in [
        Value::Null,
        json!({"url":"https://video.example.test/input.mp4"}),
        json!({"data":"YQ==","media_type":"image/png"}),
        json!({"generation_id":"wrong"}),
    ] {
        let mut value = body();
        value["source"] = source;
        assert!(pack(value).is_err());
    }
}

#[test]
fn luma_video_edit_preserves_auto_manual_strength_and_signal_controls() {
    for strength in [
        "adhere_1",
        "adhere_2",
        "adhere_3",
        "flex_1",
        "flex_2",
        "flex_3",
        "reimagine_1",
        "reimagine_2",
        "reimagine_3",
    ] {
        let value = with_video(json!({"edit":{"strength":strength}}));
        assert_eq!(pack(value.clone()).unwrap().body["video"], value["video"]);
    }
    for edit in [
        json!({"auto_controls":true}),
        json!({"auto_controls":false,"controls":{}}),
        json!({"controls":{"pose":{"enabled":true,"strength":"precise"},"depth":{"blur":0},
            "normals":{"augmentation":1.0},"trajectory":{"sparsity":0.25},"face":{"enabled":false}}}),
    ] {
        let value = with_video(json!({"resolution":"720p","edit":edit}));
        assert_eq!(pack(value.clone()).unwrap().body["video"], value["video"]);
    }
}

#[test]
fn luma_video_edit_rejects_bad_signal_types_bounds_unknown_controls_and_auto_conflict() {
    for edit in [
        json!({"strength":"flex_4"}),
        json!({"auto_controls":"true"}),
        json!({"auto_controls":true,"controls":{}}),
        json!({"controls":[]}),
        json!({"unknown":true}),
        json!({"controls":{"other":{}}}),
        json!({"controls":{"pose":{"enabled":"true"}}}),
        json!({"controls":{"pose":{"strength":"flex_2"}}}),
        json!({"controls":{"depth":{"blur":-0.01}}}),
        json!({"controls":{"depth":{"blur":1.01}}}),
        json!({"controls":{"normals":{"augmentation":"1"}}}),
        json!({"controls":{"trajectory":{"sparsity":false}}}),
        json!({"controls":{"face":{"strength":1}}}),
        json!({"controls":{"face":true}}),
    ] {
        assert!(pack(with_video(json!({"edit":edit}))).is_err(), "{edit}");
    }
}

#[test]
fn luma_video_edit_accepts_source_frame_guides_without_inventing_source_length() {
    let frame = json!({"url":"https://image.example.test/guide.png"});
    let single = with_video(json!({"start_frame":frame,"edit":{"auto_controls":true}}));
    assert_eq!(pack(single.clone()).unwrap().body["video"], single["video"]);
    let multiple = with_video(
        json!({"edit":{"keyframes":[frame,{"generation_id":"d290f1ee-6c54-4b01-90e6-d701748f0851"}],"keyframe_indexes":[0,90]}}),
    );
    assert_eq!(
        pack(multiple.clone()).unwrap().body["video"],
        multiple["video"]
    );
    let max = with_video(
        json!({"edit":{"keyframes":vec![frame;64],"keyframe_indexes":(0..64).collect::<Vec<_>>()}}),
    );
    assert!(pack(max).is_ok());
}

#[test]
fn luma_video_edit_rejects_unpaired_duplicate_bad_frames_and_mixed_anchor_forms() {
    let frame = json!({"url":"https://image.example.test/guide.png"});
    for edit in [
        json!({"keyframes":[frame]}),
        json!({"keyframe_indexes":[0]}),
        json!({"keyframes":[],"keyframe_indexes":[]}),
        json!({"keyframes":[frame],"keyframe_indexes":[0,1]}),
        json!({"keyframes":[frame,frame],"keyframe_indexes":[1,1]}),
        json!({"keyframes":[frame],"keyframe_indexes":[-1]}),
        json!({"keyframes":[frame],"keyframe_indexes":[0.5]}),
        json!({"keyframes":[frame],"keyframe_indexes":["0"]}),
        json!({"keyframes":[{"file_id":"bad"}],"keyframe_indexes":[0]}),
        json!({"keyframes":vec![frame.clone();65],"keyframe_indexes":(0..65).collect::<Vec<_>>()}),
    ] {
        assert!(pack(with_video(json!({"edit":edit}))).is_err(), "{edit}");
    }
    assert!(pack(with_video(
        json!({"start_frame":frame,"edit":{"keyframes":[frame],"keyframe_indexes":[0]}})
    ))
    .is_err());
}

#[test]
fn luma_video_edit_hdr_is_standard_video_only_and_unfulfillable_controls_reject() {
    for video in [
        json!({"hdr":true}),
        json!({"hdr":true,"resolution":"720p"}),
        json!({"hdr":true,"resolution":"1080p"}),
    ] {
        assert_eq!(
            pack(with_video(video.clone())).unwrap().body["video"],
            video
        );
    }
    for video in [
        json!({"hdr":true,"resolution":"360p"}),
        json!({"hdr":true,"resolution":"540p"}),
        json!({"hdr":"true"}),
        json!({"exr_export":true}),
        json!({"duration":"10s"}),
        json!({"loop":false}),
        json!({"end_frame":{}}),
        json!({"keyframes":[]}),
        json!({"source_position":{}}),
    ] {
        assert!(pack(with_video(video)).is_err());
    }
    for field in ["aspect_ratio", "image_ref", "style"] {
        let mut value = body();
        value[field] = json!("unsupported");
        assert!(pack(value).is_err());
    }
    let mut canonical =
        crate::protocol::gemini_canvas::normalize_video_generations(body()).unwrap();
    assert!(GenerationRequest::pack(&canonical, "uni-1").is_err());
    canonical.endpoint_kind = crate::protocol::canonical::EndpointKind::ImagesEdits;
    assert!(GenerationRequest::pack(&canonical, "uni-1").is_err());
}
