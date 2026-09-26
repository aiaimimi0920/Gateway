use super::*;

use crate::protocol::chatgpt::web_reverse::{
    CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE, CHATGPT_WEB_SESSION_INVALID_CODE,
};

#[test]
fn classify_chatgpt_web_http_error_detects_browser_challenge() {
    let error = classify_chatgpt_web_http_error(
        403,
        Some("text/html; charset=utf-8"),
        "<!doctype html><html><body>Just a moment... verify you are human</body></html>",
    );
    assert_eq!(
        error.code.as_deref(),
        Some(CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE)
    );
    assert_eq!(error.http_status, Some(403));
}

#[test]
fn classify_chatgpt_web_http_error_detects_session_invalid() {
    let error = classify_chatgpt_web_http_error(
        401,
        Some("text/html; charset=utf-8"),
        "<!doctype html><html><body>session expired, please log in again</body></html>",
    );
    assert_eq!(
        error.code.as_deref(),
        Some(CHATGPT_WEB_SESSION_INVALID_CODE)
    );
    assert_eq!(error.http_status, Some(401));
}

#[test]
fn accumulate_response_errors_when_sse_frames_are_missing() {
    let error = accumulate_response("plain text only", "gpt-5").expect_err("invalid sse");
    assert_eq!(error.code.as_deref(), Some("chatgpt_web_invalid_sse"));
}

#[test]
fn accumulate_response_errors_when_stream_is_blocked() {
    let body = concat!(
        "data: {\"type\":\"moderation\",\"moderation_response\":{\"blocked\":true}}\n\n",
        "data: [DONE]\n\n"
    );
    let error = accumulate_response(body, "gpt-5").expect_err("blocked");
    assert_eq!(error.code.as_deref(), Some("chatgpt_web_blocked"));
}

#[test]
fn accumulate_response_errors_when_text_never_arrives() {
    let body = concat!("data: \"v1\"\n\n", "data: [DONE]\n\n");
    let error = accumulate_response(body, "gpt-5").expect_err("missing text");
    assert_eq!(error.code.as_deref(), Some("chatgpt_web_missing_text"));
}

#[test]
fn accumulate_response_extracts_assistant_delta_text() {
    let body = concat!(
        "data: \"v1\"\n\n",
        "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Hello\"}\n\n",
        "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\" world\"}\n\n",
        "data: [DONE]\n\n"
    );
    let response = accumulate_response(body, "gpt-5").expect("canonical");
    assert_eq!(response.text, "Hello world");
    assert_eq!(response.finish_reason.as_deref(), Some("stop"));
}

#[test]
fn accumulate_response_ignores_plain_status_frames() {
    let body = concat!(
        "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\n",
        "data: finished_successfully\n\n",
        "data: [DONE]\n\n"
    );
    let response = accumulate_response(body, "gpt-5").expect("canonical");
    assert_eq!(response.text, "Paris");
}

#[test]
fn accumulate_response_ignores_json_string_status_frames() {
    let body = concat!(
        "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\n",
        "data: \"finished_successfully\"\n\n",
        "data: [DONE]\n\n"
    );
    let response = accumulate_response(body, "gpt-5").expect("canonical");
    assert_eq!(response.text, "Paris");
}

#[test]
fn accumulate_response_ignores_web_annotation_control_frames() {
    let body = concat!(
        "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\n",
        "data: \"\u{e200}entity\u{e202}[\\\"city\\\",\\\"Paris\\\"]\u{e201}\"\n\n",
        "data: \"doneE-D1snLkeVaVIyv6\"\n\n",
        "data: [DONE]\n\n"
    );
    let response = accumulate_response(body, "gpt-5").expect("canonical");
    assert_eq!(response.text, "Paris");
}

#[test]
fn accumulate_response_replaces_text_from_nested_assistant_message() {
    let body = concat!(
        "data: {\"message\":{\"author\":{\"role\":\"assistant\"},\"content\":{\"parts\":[\"draft\"]}}}\n\n",
        "data: {\"p\":\"/message/content/parts/0\",\"o\":\"replace\",\"v\":\"final\"}\n\n",
        "data: [DONE]\n\n"
    );
    let response = accumulate_response(body, "gpt-5").expect("canonical");
    assert_eq!(response.text, "final");
}

#[test]
fn translate_to_openai_sse_serializes_text_and_done_chunk() {
    let body = concat!(
        "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"hello\"}\n\n",
        "data: [DONE]\n\n"
    );
    let chunks = translate_to_openai_sse(body, "gpt-5").expect("chunks");
    assert_eq!(chunks.len(), 2);
    let first = std::str::from_utf8(chunks[0].as_ref()).expect("utf8");
    assert!(first.contains("\"content\":\"hello\""));
    let second = std::str::from_utf8(chunks[1].as_ref()).expect("utf8");
    assert_eq!(second, "data: [DONE]\n\n");
}

#[tokio::test]
async fn translate_chatgpt_web_stream_emits_incremental_chunks_and_done() {
    let upstream = futures::stream::iter(vec![
        Ok(Bytes::from_static(
            b"event: message\ndata: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"hello\"}\n\n",
        )),
        Ok(Bytes::from_static(
            b"event: message\ndata: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\" world\"}\n\n",
        )),
        Ok(Bytes::from_static(b"data: [DONE]\n\n")),
    ]);

    let chunks = translate_chatgpt_web_stream(upstream, "gpt-5.4".to_string())
        .collect::<Vec<_>>()
        .await;
    let rendered = chunks
        .into_iter()
        .map(|item| String::from_utf8(item.expect("ok").to_vec()).expect("utf8"))
        .collect::<Vec<_>>();

    assert!(rendered
        .iter()
        .any(|item| item.contains("\"content\":\"hello\"")));
    assert!(rendered
        .iter()
        .any(|item| item.contains("\"content\":\" world\"")));
    assert!(rendered
        .iter()
        .any(|item| item.contains("\"finish_reason\":\"stop\"")));
    assert_eq!(
        rendered.last().map(String::as_str),
        Some("data: [DONE]\n\n")
    );
}

#[tokio::test]
async fn translate_chatgpt_web_stream_handles_fragmented_sse_lines() {
    let upstream = futures::stream::iter(vec![
        Ok(Bytes::from_static(b"event: message\ndata: {\"p\":\"/message/")),
        Ok(Bytes::from_static(
            b"content/parts/0\",\"o\":\"append\",\"v\":\"hel",
        )),
        Ok(Bytes::from_static(
            b"lo\"}\n\nevent: message\ndata: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\" world\"}\n",
        )),
        Ok(Bytes::from_static(b"\n")),
        Ok(Bytes::from_static(b"data: [DO")),
        Ok(Bytes::from_static(b"NE]\n\n")),
    ]);

    let chunks = translate_chatgpt_web_stream(upstream, "gpt-5.4".to_string())
        .collect::<Vec<_>>()
        .await;
    let rendered = chunks
        .into_iter()
        .map(|item| String::from_utf8(item.expect("ok").to_vec()).expect("utf8"))
        .collect::<Vec<_>>();

    assert!(rendered
        .iter()
        .any(|item| item.contains("\"content\":\"hello\"")));
    assert!(rendered
        .iter()
        .any(|item| item.contains("\"content\":\" world\"")));
    assert_eq!(
        rendered
            .iter()
            .filter(|item| item.contains("\"finish_reason\":\"stop\""))
            .count(),
        1
    );
    assert_eq!(
        rendered
            .iter()
            .filter(|item| item.as_str() == "data: [DONE]\n\n")
            .count(),
        1
    );
}

fn assert_not_html_challenge(content_type: &str) {
    for status in [200, 403, 429, 503] {
        let body = "Cloudflare challenge";
        assert!(
            !response_indicates_browser_challenge(status, Some(content_type), body),
            "{status}: {content_type}"
        );
        let error = classify_chatgpt_web_http_error(status, Some(content_type), body);
        assert_ne!(
            error.code.as_deref(),
            Some(CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE)
        );
        assert_eq!(error.http_status, Some(status));
        assert_eq!(
            error.provider_name.as_deref(),
            Some(CHATGPT_WEB_REVERSE_ADAPTER)
        );
    }
}

#[test]
fn html_mime_subtype_suffixes_are_rejected() {
    for content_type in ["text/htmlish", "text/html+json", "text/html-extra"] {
        assert_not_html_challenge(content_type);
    }
}

#[test]
fn html_mime_type_prefixes_are_rejected() {
    for content_type in ["x-text/html", "application/text/html"] {
        assert_not_html_challenge(content_type);
    }
}

#[test]
fn html_mime_parameter_tokens_are_rejected() {
    for content_type in [
        "application/json; note=TEXT/HTML",
        "text/plain; note=\"text/html\"",
        "application/octet-stream; text/html",
    ] {
        assert_not_html_challenge(content_type);
    }
}

#[test]
fn html_mime_comma_lists_are_rejected() {
    for content_type in ["text/html, application/json", "application/json, text/html"] {
        assert_not_html_challenge(content_type);
    }
}

#[test]
fn html_mime_parameters_preserve_challenge_contract() {
    for content_type in [
        "text/html",
        "TEXT/HTML ; charset=utf-8",
        "text/html\t; charset=\"utf-8\"; note=\"a;b\"",
    ] {
        for status in [200, 403, 429, 503] {
            let error = classify_chatgpt_web_http_error(
                status,
                Some(content_type),
                "Just a moment: verify you are human",
            );
            assert_eq!(
                error.code.as_deref(),
                Some(CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE)
            );
            assert_eq!(error.kind, crate::error::ErrorKind::ServiceUnavailable);
            assert_eq!(error.http_status, Some(status));
            assert_eq!(
                error.provider_name.as_deref(),
                Some(CHATGPT_WEB_REVERSE_ADAPTER)
            );
        }
    }
}

#[test]
fn html_mime_body_sniffing_and_status_are_preserved() {
    for content_type in [None, Some("application/json"), Some("text/htmlish")] {
        assert!(response_indicates_browser_challenge(
            200,
            content_type,
            "<HTML>Cloudflare challenge</HTML>"
        ));
        assert!(response_indicates_browser_challenge(
            403,
            content_type,
            "<!DOCTYPE HTML>captcha"
        ));
    }
    for status in [201, 400, 401, 404, 500] {
        assert!(!response_indicates_browser_challenge(
            status,
            Some("text/html"),
            "<html>Cloudflare challenge</html>"
        ));
    }
    assert!(!response_indicates_browser_challenge(
        200,
        Some("text/html"),
        "<html>ordinary content</html>"
    ));
}

#[test]
fn html_mime_session_invalid_is_preserved() {
    for content_type in [
        None,
        Some("text/html"),
        Some("application/json; note=\"text/html\""),
    ] {
        for status in [401, 403] {
            let error = classify_chatgpt_web_http_error(
                status,
                content_type,
                "session expired, please log in",
            );
            assert_eq!(
                error.code.as_deref(),
                Some(CHATGPT_WEB_SESSION_INVALID_CODE)
            );
            assert_eq!(error.kind, crate::error::ErrorKind::Authentication);
            assert_eq!(error.http_status, Some(status));
            assert_eq!(
                error.provider_name.as_deref(),
                Some(CHATGPT_WEB_REVERSE_ADAPTER)
            );
        }
    }
}
