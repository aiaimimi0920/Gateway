use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    build_image_generation_response, normalize_image_generations, PRODUCER_IMAGE_DEFAULT_MODEL,
};

#[test]
fn build_image_generation_response_derives_public_url_from_session_jwt() {
    let claims = URL_SAFE_NO_PAD.encode(
        json!({
            "iss": "https://demo-project.supabase.co/auth/v1",
            "sub": "user-123"
        })
        .to_string(),
    );
    let token = format!("e30.{claims}.sig");
    let req = normalize_image_generations(json!({
        "prompt": "holographic album cover",
        "type": "clip"
    }))
    .unwrap();

    let response = build_image_generation_response(
        &json!({
            "image_id": "img-42"
        }),
        &req,
        PRODUCER_IMAGE_DEFAULT_MODEL,
        Some(&token),
    )
    .unwrap();

    assert_eq!(response["object"], "image.generation");
    assert_eq!(response["data"][0]["image_id"], "img-42");
    assert_eq!(
        response["data"][0]["url"],
        "https://storage.googleapis.com/producer-app-public/assets/img-42.jpg"
    );
}

fn valid_session_jwt() -> String {
    let claims = URL_SAFE_NO_PAD.encode(json!({ "sub": "user-123" }).to_string());
    format!("e30.{claims}.sig")
}

fn image_request(image_type: &str) -> crate::protocol::canonical::CanonicalRelayRequest {
    normalize_image_generations(json!({
        "prompt": "cover art",
        "type": image_type,
    }))
    .expect("valid image request")
}

#[test]
fn build_image_generation_response_rejects_non_object_body() {
    let error = build_image_generation_response(
        &json!([]),
        &image_request("clip"),
        PRODUCER_IMAGE_DEFAULT_MODEL,
        None,
    )
    .expect_err("non-object image responses must fail");

    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_invalid_image_response")
    );
    assert_eq!(
        error.message,
        "Producer.ai image response body must be a JSON object"
    );
}

#[test]
fn build_image_generation_response_rejects_missing_or_blank_image_id() {
    for body in [json!({}), json!({ "image_id": "  " })] {
        let error = build_image_generation_response(
            &body,
            &image_request("clip"),
            PRODUCER_IMAGE_DEFAULT_MODEL,
            None,
        )
        .expect_err("missing image identifiers must fail");

        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(error.code.as_deref(), Some("producer_missing_image_id"));
        assert_eq!(error.message, "Producer.ai image response missing image_id");
    }
}

#[test]
fn build_image_generation_response_preserves_url_precedence() {
    let top_level = build_image_generation_response(
        &json!({
            "image_id": "img-top",
            "image_url": " https://cdn.example.com/top-image.jpg ",
            "url": "https://cdn.example.com/top-url.jpg",
            "data": [{ "url": "https://cdn.example.com/nested.jpg" }],
        }),
        &image_request("playlist"),
        "producer:image-custom",
        None,
    )
    .expect("top-level image URL response");
    assert_eq!(
        top_level["data"][0]["url"],
        "https://cdn.example.com/top-image.jpg"
    );
    assert_eq!(top_level["data"][0]["type"], "playlist");
    assert_eq!(top_level["model"], "producer:image-custom");
    assert!(top_level.get("upstream_response").is_none());

    let nested = build_image_generation_response(
        &json!({
            "imageId": "img-nested",
            "data": [{
                "url": " https://cdn.example.com/nested-url.jpg ",
                "image_url": "https://cdn.example.com/nested-image.jpg",
            }],
        }),
        &image_request("space"),
        PRODUCER_IMAGE_DEFAULT_MODEL,
        None,
    )
    .expect("nested image URL response");
    assert_eq!(
        nested["data"][0]["url"],
        "https://cdn.example.com/nested-url.jpg"
    );
    assert!(nested.get("upstream_response").is_none());
}

#[test]
fn build_image_generation_response_falls_back_for_invalid_auth() {
    let upstream = json!({ "image_id": "img-no-url", "opaque": { "kept": true } });
    for auth_token in [None, Some("  "), Some("not-a-jwt")] {
        let response = build_image_generation_response(
            &upstream,
            &image_request("clip"),
            PRODUCER_IMAGE_DEFAULT_MODEL,
            auth_token,
        )
        .expect("missing or invalid auth should preserve the upstream response");

        assert!(response["data"][0].get("url").is_none());
        assert_eq!(response["upstream_response"], upstream);
    }
}

#[test]
fn build_image_generation_response_falls_back_for_unknown_image_type() {
    let upstream = json!({ "image_id": "img-unknown-type" });
    let token = valid_session_jwt();
    let response = build_image_generation_response(
        &upstream,
        &image_request("avatar"),
        PRODUCER_IMAGE_DEFAULT_MODEL,
        Some(&token),
    )
    .expect("unknown image types should retain the upstream response");

    assert_eq!(response["type"], "avatar");
    assert!(response["data"][0].get("url").is_none());
    assert_eq!(response["upstream_response"], upstream);
}

#[test]
fn build_image_generation_response_preserves_or_generates_created() {
    let req = image_request("clip");
    let preserved = build_image_generation_response(
        &json!({
            "image_id": "img-created",
            "url": "https://cdn.example.com/created.jpg",
            "created": "upstream-clock",
        }),
        &req,
        PRODUCER_IMAGE_DEFAULT_MODEL,
        None,
    )
    .expect("upstream created value");
    assert_eq!(preserved["created"], "upstream-clock");

    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_secs();
    let generated = build_image_generation_response(
        &json!({
            "image_id": "img-timestamp",
            "url": "https://cdn.example.com/timestamp.jpg",
        }),
        &req,
        PRODUCER_IMAGE_DEFAULT_MODEL,
        None,
    )
    .expect("generated created timestamp");
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_secs();
    let created = generated["created"]
        .as_u64()
        .expect("generated created value is a Unix timestamp");
    assert!((before..=after).contains(&created));
}
