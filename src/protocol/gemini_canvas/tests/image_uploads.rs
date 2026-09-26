use super::*;

#[test]
fn extract_image_edit_uploads_normalizes_png_inputs_to_jpeg_contract() {
    let mut png_bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png_bytes)
        .write_image(&[0, 0, 0, 255], 1, 1, image::ExtendedColorType::Rgba8)
        .unwrap();
    let png_base64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesEdits,
        requested_model: Some(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL.to_string()),
        stream: false,
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "prompt": "edit the sample",
            "images": [{
                "mime_type": "image/png",
                "base64": png_base64
            }],
            "response_format": "url"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let uploads = extract_image_edit_uploads(&req).unwrap();
    assert_eq!(uploads.len(), 1);
    assert_eq!(uploads[0].mime_type, "image/jpeg");
    assert_eq!(uploads[0].file_name, "edit-source.jpg");
    assert!(uploads[0].bytes.starts_with(&[0xFF, 0xD8, 0xFF]));
}

#[test]
fn normalize_image_edit_upload_to_jpeg_caps_large_png_under_browser_budget() {
    let width = 1400_u32;
    let height = 1400_u32;
    let mut pixels = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            pixels.push(((x * 13 + y * 3) % 256) as u8);
            pixels.push(((x * 7 + y * 11) % 256) as u8);
            pixels.push(((x * 5 + y * 17) % 256) as u8);
        }
    }
    let mut png_bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png_bytes)
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgb8)
        .unwrap();

    let jpeg_bytes = normalize_image_edit_upload_to_jpeg("image/png", &png_bytes).unwrap();
    assert!(jpeg_bytes.starts_with(&[0xFF, 0xD8, 0xFF]));
    assert!(
        jpeg_bytes.len() <= 127_600,
        "expected browser-budget jpeg, got {} bytes",
        jpeg_bytes.len()
    );
    assert!(
        jpeg_bytes.len() >= 124_000,
        "expected search to stay near browser-budget jpeg, got {} bytes",
        jpeg_bytes.len()
    );
}
