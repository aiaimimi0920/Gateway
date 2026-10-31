//! Synthetic loopback only: never launches a worker or reads account storage.
use super::*;
use axum::{routing::post, Router};
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[path = "image_send_once_tests.rs"]
mod heavy;
#[path = "image_recovery_tests.rs"]
mod recovery;

fn payload(base: &str) -> ProviderAccountPayload {
    serde_json::from_value(json!({
        "adapter":"gemini_web_reverse_modular_compatible", "base_url":base, "api_key":"fixture",
    }))
    .unwrap()
}

fn prepared(base: &str) -> StreamGeneratePreparation {
    StreamGeneratePreparation {
        effective_base_url: base.into(),
        app_bootstrap_url: format!("{base}/app"),
        share_bootstrap_url: format!("{base}/share/fixture"),
        is_text_mode: false,
        is_image_mode: true,
        is_image_edit_request: false,
        image_stream_timeout: Duration::from_secs(2),
        bootstrap_url: format!("{base}/app"),
        stream_url: format!(
            "{base}{}",
            gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH
        ),
        storage_state: json!({}),
        batchexecute_header_id: None,
        replay_template: Some(gemini_canvas::GeminiCanvasTextStreamGenerateTemplate {
            url: format!(
                "{base}{}",
                gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH
            ),
            query: vec![],
            form: vec![],
            raw_post_data: "f.req=fixture&at=old".into(),
            headers: Default::default(),
        }),
        session: gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "fixture=session".into(),
            sapisid: "fixture".into(),
            auth_user: "0".into(),
        },
    }
}

#[tokio::test]
async fn image_template_never_resends_or_refreshes_after_a_dispatched_response() {
    for (status, body) in [
        (200, "fixture complete body"),
        (200, ""),
        (200, "unparseable accepted body"),
        (400, r#"{"at":"refreshed-xsrf-fixture"}"#),
        (403, "private upstream cookie=fixture-secret"),
        (502, "private prompt"),
        (307, "redirect must not resend"),
        (308, "redirect must not resend"),
    ] {
        let hits = Arc::new(AtomicUsize::new(0));
        let observed = hits.clone();
        let app = Router::new().route(
            gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH,
            post(move || {
                observed.fetch_add(1, Ordering::SeqCst);
                async move {
                    (
                        axum::http::StatusCode::from_u16(status).unwrap(),
                        [(
                            "location",
                            gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH,
                        )],
                        body,
                    )
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let runtime = gemini_canvas::GeminiCanvasRuntime {
            share_id: "fixture".into(),
            runtime_state_object_key: "not-read".into(),
            api_base_url: base.clone(),
        };
        let mut client = UpstreamClient::new(1);
        client.http = rquest::Client::builder().no_proxy().build().unwrap();
        // Exercise the real owner choice, not just the one-POST helper.
        let result = client
            .bootstrap_gemini_canvas_stream_generate(
                prepared(&base),
                &payload(&base),
                "fixture",
                &runtime,
                gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
                Duration::from_secs(2),
                true,
                &mut None,
            )
            .await;
        server.abort();
        let _ = server.await;
        assert_eq!(hits.load(Ordering::SeqCst), 1, "status={status}");
        if status == 200 {
            assert!(
                matches!(result, Ok(StreamGenerateBootstrapOutcome::Completed(ref value)) if value.text == body)
            );
        } else {
            let error = result
                .err()
                .expect("rejection/redirect must stop before another sender");
            assert!(!error.message.contains("fixture-secret"));
            assert!(!error.message.contains("private prompt"));
        }
    }
}

#[tokio::test]
async fn image_template_rejects_cross_origin_before_sending_session_material() {
    let mut prepared = prepared("http://127.0.0.1:1");
    prepared.replay_template.as_mut().unwrap().url = format!(
        "https://fixture.invalid{}",
        gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH
    );
    let error = UpstreamClient::new(1)
        .send_gemini_web_image_template_once(
            &payload("http://127.0.0.1:1"),
            &prepared,
            prepared.replay_template.as_ref().unwrap(),
        )
        .await
        .err()
        .expect("cross-origin target must be rejected");
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_web_image_template_target_invalid")
    );
}
