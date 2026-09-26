use super::image_edit_local_helpers_test_support::make_payload;
use super::*;
use crate::upstream::gemini_canvas_upload_http::upload_gemini_canvas_image_edit_inputs_with_http;
use rquest::Method;
use serde_json::json;
use std::future::Future;
#[test]
fn upload_gemini_canvas_image_edit_inputs_with_http_returns_uploaded_refs_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<Vec<gemini_canvas::GeminiCanvasUploadedFileRef>, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "1".to_string(),
    };
    let bootstrap = crate::protocol::gemini_web::GeminiWebBootstrap {
        access_token: None,
        build_label: None,
        session_id: None,
        language: "en-US".to_string(),
        push_id: Some("push-id-123".to_string()),
        client_pctx: Some("client-pctx-456".to_string()),
        app_page_path: Some("/app".to_string()),
    };
    let uploads = Vec::<gemini_canvas::GeminiCanvasImageEditUpload>::new();

    assert_future_output(upload_gemini_canvas_image_edit_inputs_with_http(
        &http,
        &payload,
        &session,
        &bootstrap,
        &uploads,
        std::time::Duration::from_secs(1),
    ));
}

#[test]
fn send_gemini_canvas_signaler_request_refreshing_session_with_http_returns_string_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<String, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "1".to_string(),
    };

    assert_future_output(
        send_gemini_canvas_signaler_request_refreshing_session_with_http(
            &http,
            &payload,
            &mut session,
            Method::POST,
            "https://signaler-pa.clients6.google.com/punctual/v1/chooseServer?key=test",
            Some("application/json+protobuf"),
            None,
            Some("body".to_string()),
            std::time::Duration::from_secs(1),
            Some("en-US"),
        ),
    );
}

#[test]
fn send_gemini_canvas_signaler_poll_request_refreshing_session_with_http_returns_string_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<String, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "1".to_string(),
    };

    assert_future_output(
        send_gemini_canvas_signaler_poll_request_refreshing_session_with_http(
            &http,
            &payload,
            &mut session,
            "https://signaler-pa.clients6.google.com/punctual/multi-watch/channel?VER=8&RID=rpc",
            std::time::Duration::from_secs(1),
            0,
            Some("en-US"),
        ),
    );
}

#[test]
fn refresh_gemini_canvas_image_edit_signaler_creds_with_http_returns_string_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<String, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "1".to_string(),
    };
    let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
        api_key: "api-key".to_string(),
        gsession_id: "gsession-id".to_string(),
        sid: "sid".to_string(),
        next_aid: 0,
    };

    assert_future_output(refresh_gemini_canvas_image_edit_signaler_creds_with_http(
        &http,
        &payload,
        &mut session,
        &channel,
        "refresh-token",
        std::time::Duration::from_secs(1),
        Some("en-US"),
    ));
}

#[test]
fn refresh_gemini_canvas_image_edit_signaler_creds_from_body_with_http_skips_missing_token() {
    let http = rquest::Client::new();
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "1".to_string(),
    };
    let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
        api_key: "api-key".to_string(),
        gsession_id: "gsession-id".to_string(),
        sid: "sid".to_string(),
        next_aid: 0,
    };

    let refreshed = futures::executor::block_on(
        refresh_gemini_canvas_image_edit_signaler_creds_from_body_with_http(
            &http,
            &payload,
            &mut session,
            &channel,
            "long-poll body without refresh token",
            std::time::Duration::from_secs(1),
            Some("en-US"),
        ),
    )
    .expect("missing refresh token should be a successful no-op");

    assert_eq!(refreshed, None);
    assert_eq!(session.auth_user.as_str(), "1");
}

#[test]
fn open_gemini_canvas_image_edit_signaler_channel_with_http_returns_channel_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<
            Output = Result<
                crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel,
                GatewayError,
            >,
        >,
    {
    }

    let http = rquest::Client::new();
    let plain_http = rquest::Client::new();
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let runtime = gemini_canvas::GeminiCanvasRuntime {
        runtime_state_object_key: "credential-runtime/gemini-canvas/runtime.json".to_string(),
        share_id: "share".to_string(),
        api_base_url: "https://gemini.google.com".to_string(),
    };
    let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "1".to_string(),
    };

    assert_future_output(open_gemini_canvas_image_edit_signaler_channel_with_http(
        &http,
        &plain_http,
        &payload,
        &runtime,
        &mut session,
        Some("en-US"),
        std::time::Duration::from_secs(1),
    ));
}

#[test]
fn prewarm_gemini_canvas_image_edit_signaler_with_http_returns_channel_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<
            Output = Result<
                crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel,
                GatewayError,
            >,
        >,
    {
    }

    let http = rquest::Client::new();
    let plain_http = rquest::Client::new();
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let runtime = gemini_canvas::GeminiCanvasRuntime {
        runtime_state_object_key: "credential-runtime/gemini-canvas/runtime.json".to_string(),
        share_id: "share".to_string(),
        api_base_url: "https://gemini.google.com".to_string(),
    };
    let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "1".to_string(),
    };

    assert_future_output(prewarm_gemini_canvas_image_edit_signaler_with_http(
        &http,
        &plain_http,
        &payload,
        &runtime,
        &mut session,
        Some("en-US"),
        std::time::Duration::from_secs(1),
    ));
}

#[test]
fn prepare_gemini_canvas_image_edit_signaler_poll_state_with_http_returns_session_and_channel_result(
) {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<
            Output = Result<
                (
                    gemini_canvas::GeminiCanvasPureHttpSession,
                    crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel,
                ),
                GatewayError,
            >,
        >,
    {
    }

    let http = rquest::Client::new();
    let plain_http = rquest::Client::new();
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let runtime = gemini_canvas::GeminiCanvasRuntime {
        runtime_state_object_key: "credential-runtime/gemini-canvas/runtime.json".to_string(),
        share_id: "share".to_string(),
        api_base_url: "https://gemini.google.com".to_string(),
    };
    let storage_state = json!({
        "cookies": [],
    });

    assert_future_output(prepare_gemini_canvas_image_edit_signaler_poll_state_with_http(
        &http,
        &plain_http,
        &payload,
        &runtime,
        &storage_state,
        "https://gemini.google.com",
        "https://gemini.google.com/app",
        "1",
        Some("en-US"),
        None::<&crate::upstream::gemini_canvas_followup_types::GeminiCanvasImageEditFollowupContext>,
        std::time::Duration::from_secs(1),
    ));
}
