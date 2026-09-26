use super::*;

#[test]
fn gemini_canvas_image_edit_source_extension_maps_known_mime_types() {
    for (mime, extension) in [
        ("image/jpeg", "jpg"),
        (" IMAGE/JPG ", "jpg"),
        ("image/webp", "webp"),
        (" Image/GIF ", "gif"),
        ("IMAGE/BMP", "bmp"),
        ("image/png", "png"),
        ("image/unknown", "png"),
        ("", "png"),
    ] {
        assert_eq!(gemini_canvas_image_edit_source_extension(mime), extension);
    }
}

#[test]
fn build_gemini_canvas_image_edit_browser_reencode_meta_marks_used_contract() {
    let meta = build_gemini_canvas_image_edit_browser_reencode_meta(
        std::path::Path::new("gateway/scripts/gemini-canvas-image-edit-encode.mjs"),
        std::path::Path::new(".runtime/source.png"),
        std::path::Path::new(".runtime/output.jpg"),
        "encoded",
        "",
        true,
        None,
    );

    assert_eq!(
        meta["scriptPath"],
        "gateway/scripts/gemini-canvas-image-edit-encode.mjs"
    );
    assert_eq!(meta["sourcePath"], ".runtime/source.png");
    assert_eq!(meta["outputPath"], ".runtime/output.jpg");
    assert_eq!(meta["stdout"], "encoded");
    assert_eq!(meta["stderr"], "");
    assert_eq!(meta["used"], true);
    assert!(meta.get("exitCode").is_none());
}

#[test]
fn build_gemini_canvas_image_edit_browser_reencode_meta_preserves_exit_code_contract() {
    let meta = build_gemini_canvas_image_edit_browser_reencode_meta(
        std::path::Path::new("gateway/scripts/gemini-canvas-image-edit-encode.mjs"),
        std::path::Path::new(".runtime/source.png"),
        std::path::Path::new(".runtime/output.jpg"),
        "",
        "failed",
        false,
        Some(9),
    );

    assert_eq!(meta["used"], false);
    assert_eq!(meta["stderr"], "failed");
    assert_eq!(meta["exitCode"], 9);
}

#[tokio::test]
#[ignore = "requires local Node, playwright-core and an installed Chromium-compatible browser"]
async fn gemini_canvas_image_encoder_live_browser_round_trip() {
    use sha2::{Digest, Sha256};
    let source = image::RgbImage::from_pixel(32, 24, image::Rgb([180, 70, 30]));
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(source)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let encoding = reencode_gemini_canvas_image_edit_input(png.get_ref(), "image/png").await;
    let bytes = encoding
        .bytes
        .expect("real browser did not return encoded bytes");
    let metadata = encoding.metadata.expect("real browser metadata missing");
    assert_eq!(metadata["used"], true);
    assert!(metadata.get("exitCode").is_none());
    let report: Value = serde_json::from_str(metadata["stdout"].as_str().unwrap()).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["sourceWidth"], 32);
    assert_eq!(report["sourceHeight"], 24);
    assert_eq!(report["selected"]["byteLength"], bytes.len());
    assert_eq!(report["outputSha256"], hex::encode(Sha256::digest(&bytes)));
    assert_eq!(
        image::guess_format(&bytes).unwrap(),
        image::ImageFormat::Jpeg
    );
    let decoded = image::load_from_memory(&bytes).unwrap().to_rgb8();
    assert_eq!(report["selected"]["width"], decoded.width());
    assert_eq!(report["selected"]["height"], decoded.height());
    assert!((1152..=1400).contains(&decoded.width()));
    assert!((decoded.width() as i64 * 3 - decoded.height() as i64 * 4).abs() <= 2);
    let pixel = decoded.get_pixel(decoded.width() / 2, decoded.height() / 2);
    for (actual, expected) in pixel.0.into_iter().zip([180_u8, 70, 30]) {
        assert!(
            actual.abs_diff(expected) <= 8,
            "encoded pixel changed unexpectedly"
        );
    }
    let input = PathBuf::from(metadata["sourcePath"].as_str().unwrap());
    let output = PathBuf::from(metadata["outputPath"].as_str().unwrap());
    let directory = input.parent().unwrap();
    assert_eq!(output.parent(), Some(directory));
    assert_eq!(
        directory.parent(),
        Some(std::env::temp_dir().canonicalize().unwrap().as_path())
    );
    assert!(directory
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("gateway-image-encoder-"));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while directory.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "live encoder workspace was not cleaned"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
