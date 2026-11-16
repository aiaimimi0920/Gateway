//! Public image success means validated bytes; asset failures/cancellation never regenerate.
#[path = "lumalabs_pipeline/fixture.rs"]
mod fixture;
#[path = "lumalabs_pipeline/peer.rs"]
mod peer;
use crate::upstream::public_assets::within;
use base64::Engine;
use fixture::*;
use peer::{wire, Peer, Reply, ASSET};
use serde_json::Value;

#[tokio::test]
async fn luma_web_task_red_public_result_needs_original_completed_context_before_asset_fetch() {
    Box::pin(async {
        for result in [
            serde_json::json!({"signedUrl":ASSET}),
            serde_json::json!({"signedUrl":ASSET,"outputId":"one","state":"finalizing","realmId":"synthetic-realm","mediaOperation":"image","artifactField":"image","resolvedActionType":"create_image_uni_1"}),
            serde_json::json!({"signedUrl":ASSET,"outputId":"one","state":"completed","realmId":"foreign-private","mediaOperation":"image","artifactField":"image","resolvedActionType":"create_image_uni_1"}),
            serde_json::json!({"signedUrl":ASSET,"outputId":"one","state":"completed","realmId":"synthetic-realm","mediaOperation":"video","artifactField":"image","resolvedActionType":"create_image_uni_1"}),
        ] {
            let peer = Peer::start(vec![Reply::task(result),Reply::image(image::ImageFormat::Png)]).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            within(&peer.origin,async {
                let response = fixture.open(input("url")).await;
                assert!(!response.status().is_success());
                let text = drain(response.into_body()).await;
                assert!(text.contains("lumalabs_task_result") && !text.contains("foreign-private"),"{text}");
                let row=fixture.audit().await;identity(&row);assert_eq!(row.status,"failed");
                assert_eq!(row.route_trace.as_ref().unwrap()["fallbackEligible"],false);
                assert_eq!(fixture.balance().await.remaining_messages,Some(2));wire(&peer,false);
            }).await;
            fixture.finish().await;peer.stop().await;
        }
    }).await;
}

#[tokio::test]
async fn luma_web_red_pipeline_url_and_base64_verify_image_then_settle_exactly_once() {
    Box::pin(async {
        for format in ["url", "b64_json"] {
            for image_format in [image::ImageFormat::Png, image::ImageFormat::Jpeg] {
                let image = Reply::image(image_format);
                let expected = image.bytes.clone();
                let mime = image.mime;
                let peer = Peer::start(vec![Reply::executor(ASSET), image]).await;
                let fixture = Box::pin(Fixture::new(&peer)).await;
                within(&peer.origin, async {
                    let response = fixture.open(input(format)).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, axum::http::StatusCode::OK, "{text}");
                    let output: Value = serde_json::from_str(&text).unwrap();
                    if format == "url" {
                        assert_eq!(output["data"][0]["url"], ASSET);
                    } else {
                        let bytes = base64::engine::general_purpose::STANDARD
                            .decode(output["data"][0]["b64_json"].as_str().unwrap())
                            .unwrap();
                        assert_eq!(bytes, expected);
                        assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 2);
                        assert_eq!(output["data"][0]["mime_type"], mime);
                    }
                    assert!(output.get("usage").is_none());
                    assert!(
                        !text.contains("luma-synthetic-exact") && !text.contains("caller-secret")
                    );
                    let row = fixture.audit().await;
                    assert_eq!(row.status, "completed");
                    identity(&row);
                    assert_eq!(fixture.balance().await.remaining_messages, Some(1));
                    wire(&peer, true);
                })
                .await;
                fixture.finish().await;
                peer.stop().await;
            }
        }
    })
    .await;
}

#[tokio::test]
async fn luma_web_red_pipeline_html_corrupt_mime_http_and_budget_refund_without_replay() {
    Box::pin(async {
        for format in ["url", "b64_json"] {
            let mut corrupt = Reply::image(image::ImageFormat::Png);
            corrupt.bytes.truncate(30);
            let mut mismatch = Reply::image(image::ImageFormat::Png);
            mismatch.mime = "image/jpeg";
            let mut http = Reply::bytes("text/plain", b"synthetic-private".to_vec());
            http.status = 403;
            let mut redirect = Reply::bytes("text/plain", Vec::new());
            redirect.status = 302;
            for (reply, code) in [
                (
                    Reply::bytes("text/html", b"synthetic-private HTML".to_vec()),
                    "lumalabs_asset_type",
                ),
                (corrupt, "lumalabs_asset_type"),
                (mismatch, "lumalabs_asset_type"),
                (http, "lumalabs_asset_http"),
                (redirect, "lumalabs_asset_http"),
                (
                    Reply::bytes("image/png", vec![0; 16 * 1024 * 1024 + 1]),
                    "lumalabs_asset_limit",
                ),
            ] {
                let peer = Peer::start(vec![Reply::executor(ASSET), reply]).await;
                let fixture = Box::pin(Fixture::new(&peer)).await;
                within(&peer.origin, async {
                    let response = fixture.open(input(format)).await;
                    assert!(!response.status().is_success());
                    let text = drain(response.into_body()).await;
                    assert!(text.contains(code), "{text}");
                    assert!(!text.contains("synthetic-private"));
                    let row = fixture.audit().await;
                    identity(&row);
                    assert_eq!(row.status, "failed");
                    assert_eq!(row.route_trace.as_ref().unwrap()["fallbackEligible"], false);
                    assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                    wire(&peer, true);
                })
                .await;
                fixture.finish().await;
                peer.stop().await;
            }
        }
    })
    .await;
}

#[tokio::test]
async fn luma_web_red_pipeline_unsafe_target_never_fetches_or_replays() {
    Box::pin(async {
        for url in [
            "http://127.0.0.1:1/image.png",
            "https://127.0.0.1/image.png",
            "https://user:synthetic-private@assets.example.test/image.png",
            "https://assets.example.test:8443/image.png",
        ] {
            let peer = Peer::start(vec![Reply::executor(url)]).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            let response = fixture.open(input("url")).await;
            assert!(!response.status().is_success());
            let text = drain(response.into_body()).await;
            assert!(
                text.contains("lumalabs_asset_target") && !text.contains("synthetic-private"),
                "{text}"
            );
            let row = fixture.audit().await;
            identity(&row);
            assert_eq!(row.status, "failed");
            assert_eq!(fixture.balance().await.remaining_messages, Some(2));
            wire(&peer, false);
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn luma_web_red_pipeline_asset_caller_drop_closes_source_and_refunds_neutrally() {
    Box::pin(async {
        let mut held = Reply::image(image::ImageFormat::Png); held.hold = true;
        let peer = Peer::start(vec![Reply::executor(ASSET),held]).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        within(&peer.origin, async {
            let controller = fixture.state.concurrency_registry.get_or_create("luma-pipeline");
            let initial = controller.snapshot().current_limit;
            let mut call = Box::pin(fixture.open(input("url")));
            tokio::select! { _ = peer.wait_calls(2) => {}, _ = call.as_mut() => panic!("held asset must not become successful metadata"), }
            assert_eq!(fixture.balance().await.remaining_messages,Some(1)); drop(call);
            let row = fixture.audit().await; identity(&row); assert_eq!(row.status,"cancelled");
            assert_eq!(fixture.balance().await.remaining_messages,Some(2));
            assert_eq!(controller.snapshot().active_count,0); assert_eq!(controller.snapshot().current_limit,initial);
            let states = fixture.state.local_runtime.as_ref().unwrap().list_model_states(crate::db::CredentialModelStateFilters::default()).await.unwrap();
            assert!(states.iter().all(|s| s.failure_count==0)); wire(&peer,true); peer.wait_closed().await;
        }).await;
        fixture.finish().await; peer.stop().await;
    }).await;
}

#[tokio::test]
async fn luma_web_red_pipeline_invalid_response_format_is_rejected_before_executor() {
    Box::pin(async {
        let peer = Peer::start(vec![Reply::executor(ASSET)]).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        let response = fixture.open(input("private-invalid")).await;
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        let text = drain(response.into_body()).await;
        assert!(text.contains("unsupported_image_response_format"));
        assert!(
            peer.calls().is_empty(),
            "invalid response format must not consume remote credits"
        );
        assert_eq!(fixture.balance().await.remaining_messages, Some(2));
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}
