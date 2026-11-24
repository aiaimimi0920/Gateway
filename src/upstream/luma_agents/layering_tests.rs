//! Whole-stack metadata, cropped PNG validation, no partial output and bounded asset ownership.
use super::{layering_fixture::*, layering_output, test_support::*};
use serde_json::{json, Value};

async fn execute(server: &Server) -> Result<Value, crate::error::GatewayError> {
    let canonical = crate::protocol::luma_agents::normalize_layering(input()).unwrap();
    within(
        &server.origin,
        super::execute(
            &server.client(),
            std::time::Duration::from_secs(2),
            &payload(),
            &canonical,
            "uni-1",
        ),
    )
    .await
}
#[test]
fn luma_layering_manifest_rejects_misordered_geometry_and_text_without_dropping_metadata() {
    let original = task("completed", 2)["output"].as_array().unwrap().clone();
    let mut valid = original.clone();
    valid[0]["layer"]["contains_text"] = json!(true);
    valid[0]["layer"]["text"] = text();
    valid[0]["layer"]["unexpected_secret"] = json!("private");
    let parsed = layering_output::stack(&valid).unwrap();
    let projected = serde_json::to_value(&parsed[0]).unwrap();
    assert_eq!(projected["text"], text());
    assert!(projected.get("unexpected_secret").is_none());
    let mut absent = original.clone();
    absent[0]["layer"]["bounds"] = Value::Null;
    assert!(layering_output::stack(&absent).is_ok());
    for (pointer, value) in [
        ("/0/layer/index", json!(1)),
        ("/0/layer/index", json!(-1)),
        ("/1/layer/alpha_hint", json!("hard")),
        ("/0/layer/alpha_hint", json!("none")),
        ("/0/layer/bounds/width", json!(0)),
        ("/0/layer/bounds/x", json!(2)),
        ("/0/layer/bounds/y", json!(-1)),
        ("/1/layer/bounds/canvas_width", json!(3)),
        ("/1/layer/bounds/width", json!(1)),
        ("/0/layer/bounds/canvas_width", json!(9000)),
        ("/0/layer/contains_text", json!("true")),
        ("/0/layer/text", text()),
        ("/0/type", json!("video")),
    ] {
        let mut invalid = Value::Array(original.clone());
        *invalid.pointer_mut(pointer).unwrap() = value;
        assert!(
            layering_output::stack(invalid.as_array().unwrap()).is_err(),
            "{pointer}"
        );
    }
    for (pointer, value) in [
        ("/0/layer/text/font/weight", json!("heavy")),
        ("/0/layer/text/font/families", json!(["a", "b", "c", "d"])),
        ("/0/layer/text/font/match/weight", json!(1001)),
        ("/0/layer/text/font/match/face_id", json!("url")),
    ] {
        let mut invalid = Value::Array(valid.clone());
        *invalid.pointer_mut(pointer).unwrap() = value;
        assert!(layering_output::stack(invalid.as_array().unwrap()).is_err());
    }
    assert!(layering_output::stack(task("completed", 11)["output"].as_array().unwrap()).is_err());
    assert!(layering_output::stack(&[]).is_err());
}
#[test]
fn luma_layering_font_weight_round_trip_preserves_integer_and_fractional_numbers() {
    for weight in [json!(1), json!(700), json!(700.5), json!(1000)] {
        let mut completed = task("completed", 2);
        completed["output"][0]["layer"]["contains_text"] = json!(true);
        let mut metadata = text();
        metadata["font"]["match"]["weight"] = weight.clone();
        completed["output"][0]["layer"]["text"] = metadata;
        let parsed = layering_output::stack(completed["output"].as_array().unwrap()).unwrap();
        let projected = serde_json::to_value(&parsed[0]).unwrap();
        assert_eq!(projected["text"]["font"]["match"]["weight"], weight);
    }
    for weight in [
        json!(0),
        json!(1000.5),
        json!(-1),
        json!("700"),
        Value::Null,
    ] {
        let mut completed = task("completed", 2);
        completed["output"][0]["layer"]["contains_text"] = json!(true);
        let mut metadata = text();
        metadata["font"]["match"]["weight"] = weight;
        completed["output"][0]["layer"]["text"] = metadata;
        assert!(layering_output::stack(completed["output"].as_array().unwrap()).is_err());
    }
}
#[tokio::test]
async fn luma_layering_ten_pngs_preserve_order_bounds_and_text_with_no_auth_on_assets() {
    let mut completed = task("completed", 10);
    completed["output"][0]["layer"]["contains_text"] = json!(true);
    completed["output"][0]["layer"]["text"] = text();
    let mut sequence = replies(10);
    sequence[1] = Reply::json(200, completed.clone());
    let server = Server::start(sequence).await;
    let result = execute(&server).await.unwrap();
    assert_eq!(result["data"].as_array().unwrap().len(), 10);
    for index in 0..10 {
        assert_eq!(
            result["data"][index]["layer"],
            completed["output"][index]["layer"]
        );
        assert_eq!(result["data"][index]["mime_type"], "image/png");
        assert!(result["data"][index]["b64_json"]
            .as_str()
            .is_some_and(|s| !s.is_empty()));
    }
    let calls = server.calls();
    assert_eq!(calls.len(), 12);
    assert_eq!(calls.iter().filter(|c| c.method == "POST").count(), 1);
    assert_eq!(calls[0].body["prompt"], "");
    assert_eq!(calls[0].body["type"], "layering");
    assert_eq!(calls[1].path, format!("/v1/generations/{ID}"));
    for call in &calls[2..] {
        assert!(
            !call.headers.contains_key("authorization") && !call.headers.contains_key("cookie")
        );
    }
    server.stop().await;
}
#[tokio::test]
async fn luma_layering_bad_manifest_is_rejected_before_any_asset_download() {
    let mut completed = task("completed", 2);
    completed["output"][1]["layer"]["index"] = json!(0);
    let server = Server::start(vec![Reply::json(201, completed)]).await;
    let error = execute(&server).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("luma_agents_layers"));
    assert_eq!(server.calls().len(), 1);
    assert!(!error.retryable && error.luma_generation_failure.is_none());
    server.stop().await;
}
#[tokio::test]
async fn luma_layering_incomplete_bad_png_wrong_dimensions_and_transparent_background_are_not_success(
) {
    let mut wrong_size = image(false);
    wrong_size.bytes = image(true).bytes;
    let mut corrupt = image(false);
    corrupt.bytes = b"not png".to_vec();
    let mut rgb = image(false);
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    rgb.bytes = out.into_inner();
    let mut jpeg = image(false);
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut out, image::ImageFormat::Jpeg)
        .unwrap();
    jpeg.bytes = out.into_inner();
    jpeg.content_type = "image/jpeg";
    for asset in [wrong_size, corrupt, rgb, jpeg] {
        let server = Server::start(vec![Reply::json(201, task("completed", 2)), asset]).await;
        let error = execute(&server).await.unwrap_err();
        assert_eq!(error.code.as_deref(), Some("luma_agents_layers"));
        assert_eq!(server.calls().len(), 2);
        server.stop().await;
    }
    let mut transparent = image(true);
    transparent.bytes = Reply::png().bytes;
    let mut completed = task("completed", 1);
    completed["output"][0]["layer"]["bounds"] = Value::Null;
    let server = Server::start(vec![Reply::json(201, completed), transparent]).await;
    assert!(execute(&server).await.is_err());
    assert_eq!(server.calls().len(), 2);
    server.stop().await;
}
#[tokio::test]
async fn luma_layering_background_checks_full_precision_rgba16_alpha() {
    for alpha in [65534u16, 65535] {
        let pixels = image::ImageBuffer::from_pixel(2, 2, image::Rgba([10u16, 20, 30, alpha]));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba16(pixels)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        let mut asset = image(true);
        asset.bytes = out.into_inner();
        let server = Server::start(vec![Reply::json(201, task("completed", 1)), asset]).await;
        assert_eq!(execute(&server).await.is_ok(), alpha == 65535);
        assert_eq!(server.calls().len(), 2);
        server.stop().await;
    }
}
#[tokio::test]
async fn luma_layering_url_response_retains_manifest_without_returning_encoded_pixels() {
    let server = Server::start(replies(2)).await;
    let mut value = input();
    value["response_format"] = json!("url");
    let canonical = crate::protocol::luma_agents::normalize_layering(value).unwrap();
    let result = within(
        &server.origin,
        super::execute(
            &server.client(),
            std::time::Duration::from_secs(2),
            &payload(),
            &canonical,
            "uni-1",
        ),
    )
    .await
    .unwrap();
    assert_eq!(result["data"][0]["url"], ASSET);
    assert_eq!(result["data"][1]["layer"]["index"], 1);
    assert!(result["data"][0].get("b64_json").is_none());
    server.stop().await;
}
#[tokio::test]
async fn luma_layering_cancelled_asset_read_closes_source_without_a_background_retry() {
    let mut asset = image(false);
    asset.hold = true;
    let server = Server::start(vec![Reply::json(201, task("completed", 2)), asset]).await;
    let result =
        tokio::time::timeout(std::time::Duration::from_millis(150), execute(&server)).await;
    assert!(result.is_err());
    server.wait_closed().await;
    assert_eq!(server.calls().len(), 2);
    server.stop().await;
}
