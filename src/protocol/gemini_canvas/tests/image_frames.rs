use super::*;

#[test]
fn extract_stream_generate_media_assets_recovers_image_url() {
    let image_url = "https://example.invalid/generated.png";
    let gen_img_data = json!([[
        null,
        null,
        null,
        [
            null,
            "generation.png",
            "AI 生成",
            image_url,
            null,
            "download-token-123"
        ]
    ]]);
    let candidate_data = json!([null, null, null, null, null, null, null, [[gen_img_data]]]);
    let mut frame = vec![Value::Null; 13];
    frame[12] = candidate_data;
    let frame_json = serde_json::to_string(&vec![Value::Array(frame)]).unwrap();
    let body = format!(
        ")]}}'\n{}\n{}\n",
        frame_json.encode_utf16().count(),
        frame_json
    );

    let assets =
        extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Image).unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, "image");
    assert_eq!(assets[0].url, image_url);
    assert_eq!(assets[0].mime_type, "image/png");
    assert_eq!(
        assets[0].download_token.as_deref(),
        Some("download-token-123")
    );
}

#[test]
fn extract_stream_generate_media_assets_recovers_image_url_from_root_candidate_shape() {
    let image_url = "https://example.invalid/generated-root.png";
    let gen_img_data = json!([[null, null, null, [null, null, null, image_url]]]);
    let mut candidate_data = vec![Value::Null; 13];
    candidate_data[12] = json!({
        "7": [[gen_img_data]]
    });
    let frame_json = serde_json::to_string(&vec![Value::Array(candidate_data)]).unwrap();
    let body = format!(
        ")]}}'\n{}\n{}\n",
        frame_json.encode_utf16().count(),
        frame_json
    );

    let assets =
        extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Image).unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, "image");
    assert_eq!(assets[0].url, image_url);
    assert_eq!(assets[0].mime_type, "image/png");
}

#[test]
fn extract_stream_generate_media_assets_recovers_image_url_when_frame_length_is_invalid() {
    let image_url = "https://example.invalid/generated-invalid-length.png";
    let gen_img_data = json!([[null, null, null, [null, null, null, image_url]]]);
    let candidate_data = json!([null, null, null, null, null, null, null, [[gen_img_data]]]);
    let mut frame = vec![Value::Null; 13];
    frame[12] = candidate_data;
    let frame_json = serde_json::to_string(&vec![Value::Array(frame)]).unwrap();
    let invalid_length = frame_json.encode_utf16().count().saturating_add(7);
    let body = format!(")]}}'\n{}\n{}\n", invalid_length, frame_json);

    let assets =
        extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Image).unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, "image");
    assert_eq!(assets[0].url, image_url);
    assert_eq!(assets[0].mime_type, "image/png");
}

#[test]
fn extract_stream_generate_media_assets_recovers_image_url_when_utf16_cut_is_truncated() {
    let image_url = "https://example.invalid/generated-short-length.png";
    let gen_img_data = json!([[null, null, null, [null, null, null, image_url]]]);
    let candidate_data = json!([null, null, null, null, null, null, null, [[gen_img_data]]]);
    let mut frame = vec![Value::Null; 13];
    frame[12] = candidate_data;
    let frame_json = serde_json::to_string(&vec![Value::Array(frame)]).unwrap();
    let truncated_length = frame_json.encode_utf16().count().saturating_sub(9);
    let body = format!(")]}}'\n{}\n{}\n", truncated_length, frame_json);

    let assets =
        extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Image).unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, "image");
    assert_eq!(assets[0].url, image_url);
    assert_eq!(assets[0].mime_type, "image/png");
}

#[test]
fn extract_stream_generate_media_assets_returns_image_unavailable_for_blocked_text_body() {
    let body =
            "Are you signed in? I can search images, but I can't seem to create any images for you right now.";
    let error =
        extract_stream_generate_media_assets(body, GeminiCanvasMediaOperation::Image).unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_generation_unavailable")
    );
}

#[test]
fn extract_stream_generate_media_assets_returns_image_unavailable_for_blocked_frame_body() {
    let blocked = "I can search for images, but can't create any for you at the moment.";
    let frame_json = serde_json::to_string(&vec![Value::Array(vec![Value::String(
        blocked.to_string(),
    )])])
    .unwrap();
    let body = format!(
        ")]}}'\n{}\n{}\n",
        frame_json.encode_utf16().count(),
        frame_json
    );

    let error =
        extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Image).unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_generation_unavailable")
    );
}

#[test]
fn response_indicates_image_generation_unavailable_detects_english_blocked_message() {
    let body = "Are you signed in? I can search images, but I can't seem to create any images for you right now.";
    assert!(response_indicates_image_generation_unavailable(
        200,
        Some("application/json; charset=utf-8"),
        body,
    ));
}

#[test]
fn response_indicates_image_generation_unavailable_detects_chinese_region_message() {
    let body = "您登录了吗？我可以搜索图片，但目前似乎无法为您创建任何图片。也有可能您所在的地区尚未开通图片创建功能。";
    assert!(response_indicates_image_generation_unavailable(
        200,
        Some("application/json; charset=utf-8"),
        body,
    ));
}

#[test]
fn response_indicates_image_generation_unavailable_ignores_real_image_artifact_body() {
    let body = "image_generation_content https://lh3.googleusercontent.com/gg-dl/example";
    assert!(!response_indicates_image_generation_unavailable(
        200,
        Some("application/json; charset=utf-8"),
        body,
    ));
}
