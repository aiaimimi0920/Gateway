//! Real Udio sender with synthetic metadata and non-forwarding asset peers.
use super::*;
#[path = "udio_pipeline/peer.rs"]
mod peer;
use axum::{body::Body, http::HeaderMap, response::Response, routing::get, Router};
use std::sync::{Arc, Mutex};

struct Asset {
    origin: String,
    calls: Arc<Mutex<Vec<HeaderMap>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Asset {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Asset {
    async fn start(bytes: Vec<u8>, mime: &'static str, status: u16) -> Self {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let observed = calls.clone();
        let app = Router::new().route(
            "/image",
            get(move |headers: HeaderMap| {
                observed.lock().unwrap().push(headers);
                let bytes = bytes.clone();
                async move {
                    Response::builder()
                        .status(status)
                        .header("content-type", mime)
                        .header("location", "http://127.0.0.1/private-asset-secret")
                        .body(Body::from_stream(futures::stream::iter(
                            bytes
                                .chunks(64 * 1024)
                                .map(|chunk| {
                                    Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(chunk))
                                })
                                .collect::<Vec<_>>(),
                        )))
                        .unwrap()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            origin,
            calls,
            task,
        }
    }
    async fn stop(mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
    }
}
async fn generate(url: &str, format: &str) -> Result<Value, GatewayError> {
    let peer = peer::Peer::start(
        json!({"ok":true,"result":{"trackIds":["cover"],
        "completed":true,"songs":[{"id":"cover","finished":true,"status":"complete",
        "image_url":url}]}}),
        200,
        false,
    )
    .await;
    let mut client = UpstreamClient::new(3);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    client.browser_executor_base_url = Some(peer.origin.clone());
    client.browser_executor_bearer_token = Some("private-executor-secret".into());
    client.request_time_browser_policy = RequestTimeBrowserPolicy::RemoteOnly;
    let mut payload = make_payload("udio_compatible", "https://www.udio.com");
    payload
        .headers
        .insert("Cookie".into(), "sid=private-session-secret".into());
    let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    req.raw_body =
        json!({"model":"cover-model","prompt":"synthetic cover","response_format":format});
    let output = client
        .execute_udio_media("exact", &payload, &req, "cover-model", None)
        .await;
    assert_eq!(
        peer.calls().len(),
        1,
        "asset errors cannot regenerate or switch accounts"
    );
    peer.stop().await;
    output
}
fn rejected(output: Result<Value, GatewayError>, code: &str) {
    let error = output.expect_err("untrusted asset must not become a successful image");
    assert_eq!(error.code.as_deref(), Some(code), "{error:?}");
    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    assert!(!error.retryable);
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::Abort { .. }
    ));
    assert!(!error.message.contains("private-") && !error.message.contains("127.0.0.1"));
}

#[tokio::test]
async fn udio_image_private_target_rejected_before_reading_or_publishing() {
    let asset = Asset::start(
        b"<html>private-asset-secret</html>".to_vec(),
        "text/html",
        200,
    )
    .await;
    for format in ["url", "b64_json"] {
        rejected(
            generate(&format!("{}/image", asset.origin), format).await,
            "udio_asset_target",
        );
    }
    assert!(asset.calls.lock().unwrap().is_empty());
    asset.stop().await;
}

fn image_bytes(format: image::ImageFormat) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(2, 2)
        .write_to(&mut out, format)
        .unwrap();
    out.into_inner()
}

#[tokio::test]
async fn udio_image_url_mode_keeps_metadata_without_downloading() {
    let asset = Asset::start(b"not an image".to_vec(), "text/html", 200).await;
    let url = "https://assets.example.test/image?signature=private-url-secret";
    let result = crate::upstream::public_assets::within(&asset.origin, generate(url, "url"))
        .await
        .unwrap();
    assert_eq!(result["data"][0]["url"], url);
    assert!(
        asset.calls.lock().unwrap().is_empty(),
        "URL-only must not become a download"
    );
    asset.stop().await;
}

#[tokio::test]
async fn udio_image_base64_requires_decoded_png_jpeg_and_anonymous_reads() {
    use base64::Engine;
    for (format, mime, actual) in [
        (image::ImageFormat::Png, "image/png", "image/png"),
        (image::ImageFormat::Jpeg, "image/jpeg", "image/jpeg"),
        (
            image::ImageFormat::Png,
            "application/octet-stream",
            "image/png",
        ),
    ] {
        let bytes = image_bytes(format);
        let asset = Asset::start(bytes.clone(), mime, 200).await;
        let result = crate::upstream::public_assets::within(
            &asset.origin,
            generate("https://assets.example.test/image", "b64_json"),
        )
        .await
        .unwrap();
        assert_eq!(result["data"][0]["mime_type"], actual);
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(result["data"][0]["b64_json"].as_str().unwrap())
                .unwrap(),
            bytes
        );
        let calls = asset.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1);
        for name in ["authorization", "cookie", "origin", "referer"] {
            assert!(!calls[0].contains_key(name));
        }
        asset.stop().await;
    }
}

#[tokio::test]
async fn udio_image_invalid_content_never_becomes_jpeg_success() {
    let png = image_bytes(image::ImageFormat::Png);
    for (bytes, mime) in [
        (b"<html>private-asset-secret</html>".to_vec(), "text/html"),
        (Vec::new(), "image/jpeg"),
        (png[..20].to_vec(), "image/png"),
        (png.clone(), "image/jpeg"),
        (png, "text/plain"),
    ] {
        let asset = Asset::start(bytes, mime, 200).await;
        rejected(
            crate::upstream::public_assets::within(
                &asset.origin,
                generate("https://assets.example.test/image", "b64_json"),
            )
            .await,
            "udio_asset_type",
        );
        assert_eq!(asset.calls.lock().unwrap().len(), 1);
        asset.stop().await;
    }
}

#[tokio::test]
async fn udio_image_http_failures_do_not_redirect_leak_or_regenerate() {
    for status in [302, 403, 500] {
        let asset = Asset::start(b"private-asset-secret".to_vec(), "text/html", status).await;
        let result = crate::upstream::public_assets::within(
            &asset.origin,
            generate(
                "https://assets.example.test/image?token=private-url-secret",
                "b64_json",
            ),
        )
        .await;
        assert_eq!(result.as_ref().unwrap_err().http_status, Some(502));
        rejected(result, "udio_asset_http");
        assert_eq!(asset.calls.lock().unwrap().len(), 1);
        asset.stop().await;
    }
}

#[tokio::test]
async fn udio_image_chunked_read_has_a_decoded_byte_limit() {
    let asset = Asset::start(vec![0; 16 * 1024 * 1024 + 1], "image/png", 200).await;
    rejected(
        crate::upstream::public_assets::within(
            &asset.origin,
            generate("https://assets.example.test/image", "b64_json"),
        )
        .await,
        "udio_asset_limit",
    );
    assert_eq!(asset.calls.lock().unwrap().len(), 1);
    asset.stop().await;
}

#[tokio::test]
async fn udio_image_unsafe_schemes_ports_and_userinfo_are_rejected() {
    for url in [
        "file:///private-file",
        "http://assets.example.test/image",
        "https://[::1]/image",
        "https://169.254.169.254/image",
        "https://user:private-secret@assets.example.test/image",
        "https://assets.example.test:8443/image",
        "https://assets.example.test/image#fragment",
    ] {
        rejected(generate(url, "url").await, "udio_asset_target");
    }
}
