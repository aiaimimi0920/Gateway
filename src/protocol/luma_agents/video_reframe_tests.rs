use super::*;
use serde_json::{json, Value};

fn body() -> Value {
    json!({"model":"alias", "prompt":"向两侧扩画", "type":"video_reframe", "aspect_ratio":"21:9",
        "source":{"generation_id":"d290f1ee-6c54-4b01-90e6-d701748f0851"}})
}
fn pack(body: Value) -> Result<GenerationRequest, GatewayError> {
    GenerationRequest::pack(
        &crate::protocol::gemini_canvas::normalize_video_generations(body)?,
        "ray-3.2",
    )
}

#[test]
fn luma_reframe_accepts_all_documented_source_forms_without_fetching_them() {
    for source in [
        body()["source"].clone(),
        json!({"file_id":"d290f1ee-6c54-4b01-90e6-d701748f0851"}),
        json!({"url":"https://video.example.test/input.mp4","media_type":"video/mp4"}),
        json!({"data":"YQ==","media_type":"video/webm"}),
    ] {
        let mut input = body();
        input["source"] = source.clone();
        let result = pack(input).unwrap();
        assert_eq!(result.kind, "video_reframe");
        assert_eq!(result.media, Media::Video);
        assert_eq!(result.body["model"], "ray-3.2");
        assert_eq!(result.body["source"], source);
        assert_eq!(result.body["aspect_ratio"], "21:9");
    }
}

#[test]
fn luma_reframe_preserves_rect_edges_duration_and_resolution() {
    for ratio in ["9:16", "3:4", "1:1", "4:3", "16:9", "21:9"] {
        let mut input = body();
        input["aspect_ratio"] = json!(ratio);
        input["video"] = json!({"duration":"10s", "resolution":"720p", "source_position":
            {"x_norm":-2.0,"y_norm":2.0,"w_norm":0.01,"h_norm":2.0}});
        let result = pack(input.clone()).unwrap();
        assert_eq!(result.body["video"], input["video"]);
    }
}

#[test]
fn luma_reframe_rejects_wrong_source_mime_and_ambiguous_or_invalid_references() {
    for source in [
        Value::Null,
        json!({}),
        json!({"url":"https://v.example.test/x"}),
        json!({"url":"https://v.example.test/x","media_type":"image/png"}),
        json!({"url":"http://v.example.test/x","media_type":"video/mp4"}),
        json!({"url":"https://user:secret@v.example.test/x","media_type":"video/mp4"}),
        json!({"data":"?","media_type":"video/mp4"}),
        json!({"data":"YQ==","media_type":"video/*"}),
        json!({"data":"YQ==","media_type":"video/mp4\r\nsecret:x"}),
        json!({"file_id":"../video"}),
        json!({"generation_id":"d290f1ee-6c54-4b01-90e6-d701748f0851","url":"https://v.example.test/x"}),
        json!({"file_id":"d290f1ee-6c54-4b01-90e6-d701748f0851","media_type":"video/mp4"}),
    ] {
        let mut input = body();
        input["source"] = source;
        assert!(pack(input).is_err());
    }
}

#[test]
fn luma_reframe_rejects_unsupported_operation_controls_and_vertical_1080p() {
    for extra in [
        json!({"aspect_ratio":null}),
        json!({"aspect_ratio":"2:3"}),
        json!({"video":{"duration":"20s"}}),
        json!({"video":{"resolution":"4k"}}),
        json!({"video":{"hdr":false}}),
        json!({"video":{"loop":false}}),
        json!({"video":{"edit":{}}}),
        json!({"video":{"exr_export":true}}),
        json!({"video":{"start_frame":body()["source"]}}),
        json!({"video":{"keyframes":[]}}),
        json!({"image_ref":[]}),
        json!({"aspect_ratio":"9:16","video":{"resolution":"1080p"}}),
        json!({"aspect_ratio":"3:4","video":{"resolution":"1080p"}}),
    ] {
        let mut input = body();
        input
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert!(pack(input.clone()).is_err(), "{input}");
    }
    let mut input = body();
    input["video"] = json!({"resolution":"1080p"});
    assert!(pack(input).is_ok());
}

#[test]
fn luma_reframe_rejects_bad_rectangles_and_cross_operation_or_model_use() {
    let rect = json!({"x_norm":0,"y_norm":0,"w_norm":1,"h_norm":1});
    for (key, value) in [
        ("x_norm", json!(-2.01)),
        ("y_norm", json!(2.01)),
        ("w_norm", json!(0)),
        ("h_norm", json!(2.01)),
        ("x_norm", json!("0")),
        ("h_norm", Value::Null),
        ("extra", json!(1)),
    ] {
        let mut bad = rect.clone();
        bad[key] = value;
        let mut input = body();
        input["video"] = json!({"source_position":bad});
        assert!(pack(input).is_err());
    }
    let canonical = crate::protocol::gemini_business::normalize_image_generations(body()).unwrap();
    assert!(GenerationRequest::pack(&canonical, "uni-1").is_err());
    let canonical = crate::protocol::gemini_canvas::normalize_video_generations(body()).unwrap();
    assert!(GenerationRequest::pack(&canonical, "uni-1").is_err());
    let mut input = body();
    input["type"] = json!("video");
    assert!(pack(input).is_err());
}
