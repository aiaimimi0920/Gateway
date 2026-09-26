use super::*;
use serde_json::Value;

fn framed_body(payload: &Value) -> String {
    let frame_json = serde_json::to_string(payload).unwrap();
    let frame_len = frame_json.encode_utf16().count();
    format!("{frame_len}\n{frame_json}\n")
}

fn framed_body_crlf(payload: &Value) -> String {
    let frame_json = serde_json::to_string(payload).unwrap();
    let frame_len = frame_json.encode_utf16().count();
    format!("{frame_len}\r\n{frame_json}\r\n")
}

fn framed_item_body(payloads: &[Value]) -> String {
    payloads
        .iter()
        .map(|payload| framed_body(&json!([payload])))
        .collect::<Vec<_>>()
        .join("")
}

#[test]
fn session_invalid_ignores_gemini_shell_with_signin_promo() {
    let html = r#"
<!doctype html><html><head>
<script src="https://gemini.gstatic.com/_/mss/boq-bard-web/_/js/k=boq-bard-web.BardChatUi.en_US.test/am=1/d=1/rs=test"></script>
</head><body>
<a aria-label="Sign in" href="https://accounts.google.com/ServiceLogin?continue=https://gemini.google.com/app">Sign in</a>
</body></html>
"#;
    assert!(!response_indicates_session_invalid(
        200,
        Some("text/html; charset=utf-8"),
        html,
    ));
}

#[test]
fn session_invalid_still_detects_plain_auth_gate_html() {
    let html = r#"
<!doctype html><html><body>
<a href="https://accounts.google.com/ServiceLogin">Sign in</a>
</body></html>
"#;
    assert!(response_indicates_session_invalid(
        200,
        Some("text/html; charset=utf-8"),
        html,
    ));
}

#[test]
fn browser_challenge_ignores_gemini_shell_with_incidental_challenge_text() {
    let html = r#"
<!doctype html><html><head>
<script src="https://gemini.gstatic.com/_/mss/boq-bard-web/_/js/k=boq-bard-web.BardChatUi.en_US.test"></script>
<script>window.challengeConfiguration = {};</script>
</head><body>Gemini</body></html>
"#;
    assert!(!response_indicates_browser_challenge(
        200,
        Some("text/html; charset=utf-8"),
        html,
    ));
}

#[test]
fn browser_challenge_still_detects_plain_captcha_html() {
    let html = r#"
<!doctype html><html><body>
<h1>Verify you are human</h1><div class="captcha">Security check</div>
</body></html>
"#;
    assert!(response_indicates_browser_challenge(
        200,
        Some("text/html; charset=utf-8"),
        html,
    ));
}

#[test]
fn accumulates_direct_text_frame_response() {
    let body = framed_body(&json!({
        "text": "gemini web fixture ok",
        "model": "gemini-web"
    }));
    let canonical = accumulate_gemini_web_response(&body, "fallback").unwrap();
    assert_eq!(canonical.model, "gemini-web");
    assert_eq!(canonical.text, "gemini web fixture ok");
}

#[test]
fn accumulates_direct_text_frame_response_with_crlf() {
    let body = framed_body_crlf(&json!({
        "text": "gemini web fixture ok",
        "model": "gemini-web"
    }));
    let canonical = accumulate_gemini_web_response(&body, "fallback").unwrap();
    assert_eq!(canonical.model, "gemini-web");
    assert_eq!(canonical.text, "gemini web fixture ok");
}

#[test]
fn accumulates_plain_json_object_response_without_frames() {
    let body = serde_json::to_string(&json!({
        "text": "gemini web fixture ok",
        "model": "gemini-web"
    }))
    .unwrap();
    let canonical = accumulate_gemini_web_response(&body, "fallback").unwrap();
    assert_eq!(canonical.model, "gemini-web");
    assert_eq!(canonical.text, "gemini web fixture ok");
}

#[test]
fn accumulates_ndjson_response_without_frames() {
    let first = serde_json::to_string(&json!({
        "text": "gemini web fixture ok",
        "model": "gemini-web"
    }))
    .unwrap();
    let second = serde_json::to_string(&json!({
        "text": "ignored",
        "model": "gemini-web-2"
    }))
    .unwrap();
    let body = format!("{first}\n{second}\n");
    let canonical = accumulate_gemini_web_response(&body, "fallback").unwrap();
    assert_eq!(canonical.model, "gemini-web");
    assert_eq!(canonical.text, "gemini web fixture ok");
}

#[test]
fn accumulates_latest_incremental_canvas_frame_response() {
    let first_payload = json!([
        null,
        ["conversation-1", "response-1"],
        null,
        null,
        [["candidate-1", ["openai chat"]]]
    ]);
    let second_payload = json!([
        null,
        ["conversation-1", "response-1"],
        null,
        null,
        [["candidate-1", ["openai chat basic nonstream ok"]]],
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        "gemini-3-flash-preview"
    ]);
    let body = framed_item_body(&[
        json!([
            "wrb.fr",
            null,
            serde_json::to_string(&first_payload).unwrap()
        ]),
        json!([
            "wrb.fr",
            null,
            serde_json::to_string(&second_payload).unwrap()
        ]),
    ]);
    let canonical = accumulate_gemini_web_response(&body, "fallback").unwrap();
    assert_eq!(canonical.text, "openai chat basic nonstream ok");
}

#[test]
fn extracts_nested_candidate_text_from_inner_payload() {
    let inner = json!([null, null, null, null, [[null, ["gemini nested ok"]]]]);
    let frame = json!([null, null, serde_json::to_string(&inner).unwrap()]);
    let canonical = extract_generate_response(&frame, "fallback").unwrap();
    assert_eq!(canonical.text, "gemini nested ok");
}

#[test]
fn accumulates_direct_tool_call_frame_response_without_text() {
    let body = framed_body(&json!({
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "name": "weather",
                        "args": { "city": "Hangzhou" }
                    }
                }]
            }
        }],
        "modelVersion": "gemini-web-tool-only"
    }));
    let canonical = accumulate_gemini_web_response(&body, "fallback").unwrap();
    assert_eq!(canonical.model, "gemini-web-tool-only");
    assert_eq!(canonical.text, "");
    assert_eq!(canonical.finish_reason.as_deref(), Some("tool_calls"));
    assert_eq!(canonical.tool_calls.len(), 1);
    assert_eq!(canonical.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(
        canonical.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"Hangzhou\"}")
    );
}

#[test]
fn extracts_nested_tool_call_only_from_inner_payload() {
    let inner = json!({
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "name": "weather",
                        "args": { "city": "Hangzhou" }
                    }
                }]
            }
        }],
        "modelVersion": "gemini-web-inner-tool-only"
    });
    let frame = json!([null, null, serde_json::to_string(&inner).unwrap()]);
    let canonical = extract_generate_response(&frame, "fallback").unwrap();
    assert_eq!(canonical.model, "gemini-web-inner-tool-only");
    assert_eq!(canonical.text, "");
    assert_eq!(canonical.finish_reason.as_deref(), Some("tool_calls"));
    assert_eq!(canonical.tool_calls.len(), 1);
    assert_eq!(canonical.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(
        canonical.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"Hangzhou\"}")
    );
}

#[test]
fn translates_nonstream_response_to_openai_chunks() {
    let body = framed_body(&json!({
        "text": "gemini web fixture ok",
        "model": "gemini-web"
    }));
    let chunks = translate_gemini_web_to_openai_sse(&body, "fallback").unwrap();
    assert_eq!(chunks.len(), 2);
    let first = std::str::from_utf8(chunks[0].as_ref()).unwrap();
    assert!(first.contains("gemini web fixture ok"));
    let second = std::str::from_utf8(chunks[1].as_ref()).unwrap();
    assert_eq!(second, "data: [DONE]\n\n");
}
