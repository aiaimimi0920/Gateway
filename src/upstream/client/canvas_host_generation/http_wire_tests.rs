use super::*;

fn frame(body: Value) -> String {
    json!([[
        "wrb.fr",
        RPC,
        json!([body.to_string()]).to_string(),
        null,
        null,
        null,
        "generic"
    ]])
    .to_string()
}

#[test]
fn request_preserves_model_and_inline_inputs_without_api_credentials() {
    for (model, expected) in [
        ("gemini-3-flash-preview", 1),
        ("gemini-3.1-flash-image-preview", 4),
        ("gemini-2.5-flash-preview-tts", 5),
        ("gemini-3-pro-image", 6),
    ] {
        let body = json!({"contents":[{"parts":[{"inlineData":{"mimeType":"image/png","data":"aGVsbG8="}}]}]});
        let form = form_body(model, &body).unwrap();
        let outer: Value = serde_json::from_str(&form).unwrap();
        let request: Value = serde_json::from_str(outer[0][0][1].as_str().unwrap()).unwrap();
        assert_eq!(request[2], expected);
        let actual: Value = serde_json::from_str(request[1].as_str().unwrap()).unwrap();
        assert_eq!(actual["contents"], body["contents"]);
        assert_eq!(actual["model"], model);
        assert!(actual.get("key").is_none());
    }
    assert!(form_body("gemini-3-flash-preview", &json!({"model":"other"})).is_err());
    assert!(operation("model/unsafe?key=secret").is_err());
    assert!(operation("lyria-realtime-exp").is_err());
}

#[test]
fn response_requires_the_correct_rpc_and_decoded_generate_content() {
    let body = json!({"candidates":[{"content":{"parts":[{"text":"fresh-answer"}]}}]});
    let response = frame(body.clone());
    assert_eq!(
        parse_response(&format!(")]}}'\n\n{}\n{response}\n", response.len())).unwrap(),
        body
    );
    assert!(parse_response(&response.replace(RPC, "otherRpc")).is_err());
    assert!(parse_response(&format!("{response}\n{response}")).is_err());
    for malformed in [
        "[]",
        "<html>private-upstream-sentinel</html>",
        r#"[["wrb.fr","q4uTj","secret-upstream-sentinel"]]"#,
    ] {
        let error = parse_response(malformed).unwrap_err();
        assert!(!error.message.contains("private-upstream-sentinel"));
        assert!(!error.message.contains("secret-upstream-sentinel"));
    }
}

#[test]
fn internal_errors_are_not_mistaken_for_outer_http_success_or_logged_raw() {
    for (code, expected) in [
        (16, "gemini_canvas_http_session_invalid"),
        (8, "gemini_canvas_http_quota_exhausted"),
        (7, "gemini_canvas_http_permission_denied"),
        (13, "gemini_canvas_http_rpc_failed"),
    ] {
        let raw = json!([["wrb.fr", RPC, null, null, null, [code], "private-secret"]]).to_string();
        let error = parse_response(&raw).unwrap_err();
        assert_eq!(error.code.as_deref(), Some(expected));
        assert!(!error.message.contains("private-secret"));
    }
    assert!(parse_response(&frame(json!({"error":{"message":"private-secret"}}))).is_err());
}
