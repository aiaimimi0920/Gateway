//! A diagnostic on stderr must neither invalidate stdout nor disclose capture material.
use super::parse_gemini_web_capture_output;
use serde_json::json;

fn successful_capture() -> serde_json::Value {
    json!({"ok": true, "apiKey": "synthetic-private-cookie", "responseStatus": 200,
        "responseContainsParis": true, "cookieHeader": "SID=synthetic-private-cookie"})
}

#[test]
fn successful_stdout_survives_stderr_diagnostics() {
    let stdout = format!("[capture] Opening browser\n{}\n", successful_capture());
    let parsed = parse_gemini_web_capture_output(&stdout, "Debugger attached.\n", true)
        .expect("stderr diagnostics must not corrupt a successful stdout result");
    assert_eq!(parsed.api_key.as_deref(), Some("synthetic-private-cookie"));
}

#[test]
fn parse_errors_never_echo_raw_capture_material() {
    for stdout in [
        "synthetic-private-cookie { broken-json".to_string(),
        json!({"ok": true, "apiKey": {"secret": "synthetic-private-cookie"}}).to_string(),
    ] {
        let error =
            parse_gemini_web_capture_output(&stdout, "synthetic-private-cookie stderr", true)
                .expect_err("malformed capture must fail safely");
        assert!(!error.contains("synthetic-private-cookie"), "{error}");
        assert!(!error.contains("broken-json"), "{error}");
    }
}

#[test]
fn session_cookie_header_rejects_crlf_and_oversize() {
    for cookie in [
        "SID=value\r\nInjected: value".to_string(),
        "x".repeat(16 * 1024 + 1),
    ] {
        let mut capture = successful_capture();
        capture["cookieHeader"] = json!(cookie);
        let error = parse_gemini_web_capture_output(&capture.to_string(), "", true).unwrap_err();
        assert!(error.contains("invalid Gemini Web session cookies"));
    }
}

#[test]
fn valid_json_from_failed_worker_is_not_success() {
    let error = parse_gemini_web_capture_output(&successful_capture().to_string(), "", false)
        .expect_err("a failed worker cannot import credentials");
    assert!(!error.contains("synthetic-private-cookie"));
}
