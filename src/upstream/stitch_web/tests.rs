use super::*;
use crate::protocol::openai;
use rquest::header::{HeaderMap, HeaderValue, COOKIE, SET_COOKIE};
use serde_json::json;

fn bundle() -> serde_json::Value {
    json!({"schemaVersion":1,"provider":"stitch-web","authUser":"0","cookies":[{
        "name":"OSID","value":"synthetic","domain":"stitch.withgoogle.com","path":"/","secure":true,"expires":-1
    }]})
}

#[test]
fn stitch_web_cookie_scopes_prefixes_rotation_and_https_are_preserved() {
    let mut value = bundle();
    value["cookies"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"ordinary","value":"fixture",
        "domain":".withgoogle.com","path":"/projects","secure":false,"expires":-1}));
    let mut jar =
        session::StitchCookies::from_bundle(&serde_json::to_vec(&value).unwrap()).unwrap();
    let mut h = HeaderMap::new();
    jar.apply(ORIGIN, &mut h).unwrap();
    assert!(!h[COOKIE].to_str().unwrap().contains("ordinary"));
    jar.apply(&format!("{ORIGIN}/projects/123"), &mut h)
        .unwrap();
    assert!(h[COOKIE].to_str().unwrap().contains("ordinary=fixture"));
    for url in [
        "http://stitch.withgoogle.com",
        "https://evil.withgoogle.com",
        "https://user:secret@stitch.withgoogle.com",
    ] {
        assert!(jar.apply(url, &mut h).is_err());
    }
    let mut updates = HeaderMap::new();
    updates.insert(
        SET_COOKIE,
        HeaderValue::from_static("OSID=rotated; Secure; HttpOnly; Path=/"),
    );
    jar.receive(ORIGIN, &updates).unwrap();
    let snapshot: serde_json::Value = serde_json::from_slice(&jar.bundle_bytes().unwrap()).unwrap();
    assert!(snapshot["cookies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == "OSID" && c["value"] == "rotated"));
    for (field, value) in [
        ("domain", json!(".google.com")),
        ("path", json!("/; injected")),
        ("value", json!("secret\r\nX: injected")),
    ] {
        let mut material = bundle();
        material["cookies"][0][field] = value;
        assert!(
            session::StitchCookies::from_bundle(&serde_json::to_vec(&material).unwrap()).is_err()
        );
    }
    let mut material = bundle();
    material["cookies"][0]["name"] = json!("__Secure-invalid");
    material["cookies"][0]["secure"] = json!(false);
    assert!(session::StitchCookies::from_bundle(&serde_json::to_vec(&material).unwrap()).is_err());
}

#[test]
fn stitch_web_only_returns_real_scoped_html_artifacts_not_pending_or_external_payloads() {
    let screen = json!([
        [null, null, "https://lh3.googleusercontent.com/fixture"],
        [
            null,
            null,
            "https://contribution.usercontent.google.com/download?id=fixture",
            null,
            null,
            "text/html"
        ],
        null,
        null,
        "screen-id",
        "agent",
        "800",
        "600",
        "Task list"
    ]);
    let text = result::manifest("projects/123", &json!([[screen.clone()]])).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(manifest["screens"][0]["id"], "screen-id");
    assert_eq!(manifest["screens"][0]["width"], 800);
    assert_eq!(manifest["screens"][0]["height"], 600);
    assert!(result::manifest("projects/123", &json!([])).is_err());
    let mut bad = screen;
    bad[1][2] = json!("https://evil.invalid/secret");
    assert!(result::manifest("projects/123", &json!([[bad]])).is_err());
}

#[tokio::test]
async fn stitch_web_rejects_unsupported_input_or_origin_before_any_credential_or_network_access() {
    let client = crate::upstream::client::UpstreamClient::new(1);
    let payload: ProviderAccountPayload = serde_json::from_value(
        json!({"base_url":ORIGIN,"adapter":"stitch_web_compatible","api_key":"",
        "call_flow":{"special_adapter":"stitch_web","fingerprint_adapter":"session"}}),
    )
    .unwrap();
    let request = openai::normalize_chat_completions(
        json!({"model":MODEL,"messages":[{"role":"user","content":"A task list UI"}]}),
    )
    .unwrap();
    let error = client
        .execute(&payload, &request, MODEL, None)
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("stitch_web_session_missing"));
    assert!(!error.retryable);
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::Abort { .. }
    ));
    let error = client
        .execute(&payload, &request, "gemini-web-chat", None)
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("stitch_web_input_unsupported"));
    let boundary = openai::normalize_chat_completions(json!({"model":MODEL,"messages":[
        {"role":"user","content":"a".repeat(16384)},{"role":"user","content":"b".repeat(16384)}
    ]}))
    .unwrap();
    let error = client
        .execute(&payload, &boundary, MODEL, None)
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("stitch_web_prompt_limit"));
    let mut foreign = payload.clone();
    foreign.base_url = "https://stitch.withgoogle.com.evil.invalid".into();
    let error = client
        .execute(&foreign, &request, MODEL, None)
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("stitch_web_origin_invalid"));
}
