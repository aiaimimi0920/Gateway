//! Product material, current options, URL rejection, decoded budgets and output validity boundaries.
use super::{admission, pipeline_fixture::TOKEN, test_support::*};
use crate::{
    error::FallbackHint,
    protocol::call_flow::{ProviderCallFlow, SpecialAdapter},
    upstream::client::UpstreamClient,
};
use serde_json::json;
fn typed() -> crate::routing::candidate::ProviderAccountPayload {
    let mut payload = payload("https://chataibot.pro");
    payload.api_key = TOKEN.into();
    payload.headers.insert(
        "Cookie".into(),
        format!("token={TOKEN}; cf_clearance=synthetic"),
    );
    payload.call_flow = Some(ProviderCallFlow {
        special_adapter: SpecialAdapter::ChataibotWeb,
        ..Default::default()
    });
    payload
}
#[test]
fn chataibot_typed_material_is_fixed_origin_cookie_only_and_bound_to_its_token() {
    let headers = admission::headers(
        &typed(),
        Some(&std::collections::HashMap::from([
            ("Cookie".into(), "token=foreign-caller".into()),
            ("Authorization".into(), "Bearer foreign".into()),
        ])),
    )
    .unwrap();
    assert_eq!(
        headers["cookie"],
        format!("token={TOKEN}; cf_clearance=synthetic")
    );
    assert!(!headers.contains_key("authorization"));
    for scenario in 0..9 {
        let mut payload = typed();
        match scenario {
            0 => payload.base_url = "https://chataibot.pro.evil.test".into(),
            1 => payload.base_url = "http://chataibot.pro".into(),
            2 => payload.base_url = "https://chataibot.pro/api?target=evil".into(),
            3 => {
                payload
                    .headers
                    .insert("Cookie".into(), "token=foreign".into());
            }
            4 => {
                payload
                    .headers
                    .insert("cookie".into(), format!("token={TOKEN}"));
            }
            5 => {
                payload
                    .headers
                    .insert("Cookie".into(), format!("token={TOKEN}; token={TOKEN}"));
            }
            6 => payload.auth_token = Some("foreign-oauth".into()),
            7 => payload.expires_at = Some("2000-01-01T00:00:00Z".into()),
            8 => {
                payload.extra_body = Some(std::collections::HashMap::from([(
                    "authSeed".into(),
                    json!({}),
                )]))
            }
            _ => unreachable!(),
        }
        assert!(
            admission::headers(&payload, None).is_err(),
            "scenario {scenario}"
        );
    }
}
#[tokio::test]
async fn chataibot_bad_format_count_mime_or_base64_is_rejected_before_legacy_settings() {
    let server = Server::start(vec![]).await;
    for input in [
        json!({"response_format":"bad"}),
        json!({"n":0}),
        json!({"n":2}),
        json!({"stream":true}),
        json!({"images":[{"mime_type":"image/png\r\nX-Injected: yes","base64":"YQ=="}]}),
        json!({"images":[{"mime_type":"image/png","base64":"not base64"}]}),
    ] {
        let mut raw = json!({"prompt":"synthetic"});
        raw.as_object_mut()
            .unwrap()
            .extend(input.as_object().unwrap().clone());
        let mut request = request(raw);
        if request.raw_body.get("images").is_some() {
            request.endpoint_kind = crate::protocol::canonical::EndpointKind::ImagesEdits;
        }
        let error = UpstreamClient::new(3)
            .execute_chataibot_images(
                &payload(&server.origin),
                &request,
                "google-nano-banana-2",
                None,
            )
            .await
            .unwrap_err();
        assert!(!error.retryable && matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    }
    assert!(server.calls().is_empty());
    server.stop().await;
}
#[tokio::test]
async fn chataibot_assets_reject_unsafe_urls_mime_mismatch_truncation_and_decoded_limits() {
    for url in [
        "http://example.test/a",
        "file:///private",
        "https://127.0.0.1/a",
        "https://[::1]/a",
        "https://[::ffff:127.0.0.1]/a",
        "https://169.254.169.254/a",
        "https://user:pass@example.test/a",
        "https://assets.example.test:444/a",
        "https://assets.example.test/a#private",
    ] {
        assert!(
            super::super::public_assets::checked_url(url).is_err(),
            "{url}"
        );
    }
    for scenario in 0..4 {
        let mut asset = Reply::png();
        match scenario {
            0 => asset.mime = "image/jpeg",
            1 => asset.bytes.truncate(24),
            2 => asset.bytes = vec![0; 16 * 1024 * 1024 + 1],
            3 => asset.mime = "text/html",
            _ => unreachable!(),
        }
        let server = Server::start(vec![Reply::json(json!({"imageUrl":ASSET})), asset]).await;
        within(&server.origin, async {
            let error = UpstreamClient::new(3)
                .execute_chataibot_images(
                    &typed(),
                    &request(json!({"prompt":"synthetic"})),
                    "google-nano-banana-2",
                    None,
                )
                .await
                .unwrap_err();
            assert!(!error.retryable && matches!(error.fallback_hint, FallbackHint::Abort { .. }));
            assert!(!error.message.contains("synthetic-private"));
            assert_eq!(server.calls().len(), 2);
        })
        .await;
        server.stop().await;
    }
}
#[tokio::test]
async fn chataibot_metadata_decoded_limit_and_unknown_job_state_fail_without_replay() {
    for reply in [
        Reply {
            bytes: vec![b' '; 128 * 1024 + 1],
            ..Reply::json(json!({}))
        },
        Reply::json(json!({"status":"unknown","jobId":"synthetic"})),
        Reply::json(json!({"status":"pending","jobId":42})),
    ] {
        let server = Server::start(vec![reply]).await;
        within(&server.origin, async {
            let error = UpstreamClient::new(3)
                .execute_chataibot_images(
                    &typed(),
                    &request(json!({"prompt":"synthetic"})),
                    "google-nano-banana-2",
                    None,
                )
                .await
                .unwrap_err();
            assert!(!error.retryable && matches!(error.fallback_hint, FallbackHint::Abort { .. }));
            assert_eq!(server.calls().len(), 1);
        })
        .await;
        server.stop().await;
    }
}
