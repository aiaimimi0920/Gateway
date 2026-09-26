use super::*;
use base64::Engine;
use serde_json::json;
use std::time::Duration;

fn slots() -> Arc<Semaphore> {
    Arc::new(Semaphore::new(1))
}

fn body() -> Value {
    json!({"images": [{"mime_type": "image/png", "base64": "owned-fixture"}]})
}

#[tokio::test]
async fn normalization_empty_legacy_inputs_do_not_need_admission() {
    let slots = slots();
    let _permit = slots.clone().acquire_owned().await.unwrap();
    for body in [json!({}), json!({"images": []}), json!({"image": "legacy"})] {
        let result = normalize_with(&body, slots.clone(), |_| panic!("empty work submitted"))
            .await
            .unwrap();
        assert!(result.is_empty());
    }
}

#[tokio::test]
async fn normalization_overload_is_explicit_and_precedes_input_copy() {
    let slots = slots();
    let permit = slots.clone().acquire_owned().await.unwrap();
    let error = normalize_with(&json!({"images": [false]}), slots.clone(), |_| {
        panic!("overloaded work submitted")
    })
    .await
    .unwrap_err();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(error.code.as_deref(), Some("image_edit_normalization_busy"));
    drop(permit);
    let error = normalize_with(&json!({"images": [false]}), slots.clone(), |_| {
        panic!("malformed input submitted")
    })
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("invalid_image_upload"));
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn normalization_worker_errors_and_validation_order_survive_offload() {
    for (mime, data, code) in [
        ("not-an-image", "!", "invalid_image_edit_upload_base64"),
        (
            "text/plain",
            "aGVsbG8=",
            "invalid_image_edit_upload_mime_type",
        ),
        ("image/png", "aGVsbG8=", "invalid_image_edit_upload_image"),
    ] {
        let body = json!({"images": [{"mime_type": mime, "base64": data}]});
        let expected = gemini_canvas::normalize_image_edit_uploads(
            gemini_business::extract_uploads_from_request_body(&body).unwrap(),
        )
        .unwrap_err();
        let actual = normalize_with(&body, slots(), gemini_canvas::normalize_image_edit_uploads)
            .await
            .unwrap_err();
        assert_eq!(actual.code.as_deref(), Some(code));
        assert_eq!(actual.message, expected.message);
        assert_eq!(actual.http_status, expected.http_status);
        assert_eq!(actual.provider_name, expected.provider_name);
        assert_eq!(actual.retryable, expected.retryable);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn normalization_preserves_jpeg_bytes_sources_and_upload_order_off_thread() {
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(2, 3)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let part = json!({"mime_type": " IMAGE/PNG ", "base64":
        base64::engine::general_purpose::STANDARD.encode(png.get_ref())});
    let body = json!({"images": [part.clone(), part.clone()], "mask": part});
    let expected = gemini_canvas::normalize_image_edit_uploads(
        gemini_business::extract_uploads_from_request_body(&body).unwrap(),
    )
    .unwrap();
    let executor = std::thread::current().id();
    let actual = normalize_with(&body, slots(), move |uploads| {
        assert_ne!(executor, std::thread::current().id());
        gemini_canvas::normalize_image_edit_uploads(uploads)
    })
    .await
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 3);
    for (index, upload) in actual.iter().enumerate() {
        assert_eq!(upload.source_bytes, *png.get_ref());
        assert_eq!(upload.source_mime_type, "image/png");
        assert_eq!(upload.mime_type, "image/jpeg");
        assert!(upload.bytes.starts_with(&[255, 216, 255]));
        let expected_name = if index == 0 {
            "edit-source.jpg".to_string()
        } else {
            format!("edit-source-{}.jpg", index + 1)
        };
        assert_eq!(upload.file_name, expected_name);
    }
}

#[tokio::test]
async fn normalization_panic_returns_a_sanitized_error_and_releases_capacity() {
    let slots = slots();
    let error = normalize_with(&body(), slots.clone(), |_| {
        panic!("synthetic private worker detail")
    })
    .await
    .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("image_edit_normalization_worker_failed")
    );
    assert!(!error.message.contains("private worker detail"));
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn normalization_cancellation_keeps_owned_input_and_admission_until_exit() {
    let slots = slots();
    let worker_slots = slots.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let (finished, completion) = tokio::sync::oneshot::channel();
    let caller = tokio::spawn(async move {
        normalize_with(&body(), worker_slots, move |uploads| {
            started.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(5)).unwrap();
            finished
                .send(uploads[0].base64_data == "owned-fixture")
                .unwrap();
            Ok(Vec::new())
        })
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), ready)
        .await
        .unwrap()
        .unwrap();
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    assert_eq!(slots.available_permits(), 0);
    release.send(()).unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(5), completion)
        .await
        .unwrap()
        .unwrap());
    let permit = tokio::time::timeout(Duration::from_secs(5), slots.clone().acquire_owned())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
    assert_eq!(slots.available_permits(), 1);
}
