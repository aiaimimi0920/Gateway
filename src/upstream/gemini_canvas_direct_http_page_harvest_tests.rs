use super::direct_http_test_support::make_payload;
use super::*;
use crate::error::GatewayError;
use crate::protocol::gemini_canvas;
use crate::upstream::gemini_canvas_client_types::GeminiCanvasPageHarvestMode;
use std::future::Future;
#[test]
fn fetch_gemini_canvas_direct_http_page_html_once_returns_string_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<String, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let plain_http = rquest::Client::new();
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "0".to_string(),
    };

    assert_future_output(
        fetch_gemini_canvas_direct_http_page_html_once_refreshing_session(
            &http,
            &plain_http,
            &payload,
            &mut session,
            "https://gemini.google.com/app",
            std::time::Duration::from_secs(1),
            GeminiCanvasPageHarvestMode::AnonymousLightweightPlain,
            Some("en-US"),
        ),
    );
}

#[test]
fn fetch_gemini_canvas_direct_http_page_html_refreshing_session_returns_string_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<String, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let plain_http = rquest::Client::new();
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "0".to_string(),
    };

    assert_future_output(
        fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
            &http,
            &plain_http,
            &payload,
            &mut session,
            "https://gemini.google.com/app",
            std::time::Duration::from_secs(1),
            Some("en-US"),
        ),
    );
}
