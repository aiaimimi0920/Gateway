use super::*;
use rquest::header::{HeaderMap, HeaderValue, COOKIE, SET_COOKIE};
use serde_json::{json, Value};

fn bundle() -> Value {
    json!({"schemaVersion":1,"provider":"flow-web","authUser":"0","cookies":[{
        "name":"__Secure-1PSID","value":"synthetic-secret","domain":".google.com","path":"/",
        "secure":true,"httpOnly":true,"sameSite":"Lax","expires":-1
    }]})
}

#[test]
fn flow_web_cookie_bundle_is_scoped_bounded_and_redacted() {
    let cookie_jar =
        session::FlowCookies::from_bundle(&serde_json::to_vec(&bundle()).unwrap()).unwrap();
    let mut headers = HeaderMap::new();
    cookie_jar
        .apply("https://flow.google.com/projects", &mut headers)
        .unwrap();
    assert_eq!(headers[COOKIE], "__Secure-1PSID=synthetic-secret");
    for target in [
        "https://evil.invalid",
        "https://gemini.google.com",
        "http://flow.google.com",
        "https://user@flow.google.com",
    ] {
        assert!(cookie_jar.apply(target, &mut headers).is_err());
    }
    for (field, value) in [
        ("domain", json!(".evil.invalid")),
        ("value", json!("private\r\nCookie: injected")),
        ("expires", json!(1)),
        ("secure", json!(false)),
        ("path", json!("/other")),
    ] {
        let mut material = bundle();
        material["cookies"][0][field] = value;
        let error = session::FlowCookies::from_bundle(&serde_json::to_vec(&material).unwrap())
            .err()
            .unwrap();
        assert!(!error.message.contains("synthetic-secret"));
        assert!(!error.message.contains("injected"));
    }
    for (field, value) in [
        ("provider", json!("gemini-web")),
        ("authUser", json!("1")),
        ("origins", json!([])),
    ] {
        let mut material = bundle();
        material[field] = value;
        assert!(
            session::FlowCookies::from_bundle(&serde_json::to_vec(&material).unwrap()).is_err()
        );
    }
    assert!(session::FlowCookies::from_bundle(&vec![b'x'; 32769]).is_err());
}

#[test]
fn flow_web_cookie_rotation_preserves_paths_and_deletions() {
    let mut cookies =
        session::FlowCookies::from_bundle(&serde_json::to_vec(&bundle()).unwrap()).unwrap();
    let mut update = HeaderMap::new();
    update.append(
        SET_COOKIE,
        HeaderValue::from_static(
            "__Secure-1PSID=rotated; Domain=google.com; Path=/; Secure; HttpOnly",
        ),
    );
    update.append(
        SET_COOKIE,
        HeaderValue::from_static("rpc=path-private; Path=/_/; Secure"),
    );
    cookies
        .receive("https://flow.google.com/projects", &update)
        .unwrap();
    let mut headers = HeaderMap::new();
    cookies
        .apply("https://flow.google.com/projects", &mut headers)
        .unwrap();
    assert_eq!(headers[COOKIE], "__Secure-1PSID=rotated");
    cookies
        .apply("https://flow.google.com/_/rpc", &mut headers)
        .unwrap();
    assert!(headers[COOKIE]
        .to_str()
        .unwrap()
        .starts_with("rpc=path-private;"));
    let stored = cookies.bundle_bytes().unwrap();
    let restored = session::FlowCookies::from_bundle(&stored).unwrap();
    restored
        .apply("https://flow.google.com/projects", &mut headers)
        .unwrap();
    assert_eq!(headers[COOKIE], "__Secure-1PSID=rotated");
    restored
        .apply("https://flow.google.com/_/rpc", &mut headers)
        .unwrap();
    assert!(headers[COOKIE]
        .to_str()
        .unwrap()
        .starts_with("rpc=path-private;"));
    update.clear();
    update.insert(
        SET_COOKIE,
        HeaderValue::from_static("__Secure-1PSID=; Domain=google.com; Path=/; Secure; Max-Age=0"),
    );
    cookies
        .receive("https://flow.google.com/projects", &update)
        .unwrap();
    cookies
        .apply("https://flow.google.com/projects", &mut headers)
        .unwrap();
    assert!(headers.get(COOKIE).is_none());
}

#[test]
fn flow_web_bootstrap_and_credits_match_only_the_current_rpc() {
    let boot = wire::bootstrap(
        r#"{"FdrFJe":"-123","SNlM0e":"private\u003d"} boq_labs-ai-sandbox-frontend_20261101.01_p0"#,
    )
    .unwrap();
    assert_eq!(boot.sid, "-123");
    assert_eq!(boot.at.as_deref(), Some("private="));
    let form: std::collections::HashMap<_, _> =
        url::form_urlencoded::parse(wire::credits_form(boot.at.as_deref()).as_bytes())
            .into_owned()
            .collect();
    assert_eq!(form["at"], "private=");
    assert_eq!(
        serde_json::from_str::<Value>(&form["f.req"]).unwrap(),
        json!([[["nzlxg", "[]", null, "generic"]]])
    );
    let text = format!(
        ")]}}'\n123\n{}\n",
        json!([[
            "wrb.fr",
            "nzlxg",
            "[200,\"tier\"]",
            null,
            null,
            null,
            "generic"
        ]])
    );
    assert_eq!(wire::credits(&text).unwrap(), 200);
    for value in [
        json!([["wrb.fr", "wrong", "[200]"]]),
        json!([["er", "nzlxg", null, null, null, 401]]),
        json!([["wrb.fr", "nzlxg", null, null, null, 7]]),
        json!([["wrb.fr", "nzlxg", "[true,\"private\"]"]]),
        json!([["wrb.fr", "nzlxg", "[-1]"]]),
        json!([["wrb.fr", "nzlxg", "[null,123]"]]),
        json!([["wrb.fr", "nzlxg", "[\"unexpected\",123]"]]),
        json!([[
            "wrb.fr",
            "nzlxg",
            "[12]",
            null,
            null,
            "PUBLIC_ERROR_UNSAFE_GENERATION"
        ]]),
    ] {
        assert!(wire::credits(&value.to_string()).is_err());
    }
    assert!(wire::bootstrap("Sign in").is_err());
}

#[tokio::test]
async fn flow_web_generation_fails_closed_before_network_or_browser_without_retry() {
    let client = crate::upstream::client::UpstreamClient::new(1);
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "base_url":"https://flow.google.com", "adapter":"flow_web_compatible", "api_key":"",
        "call_flow":{"special_adapter":"flow_web","fingerprint_adapter":"session"}
    }))
    .unwrap();
    let request = crate::protocol::gemini_business::normalize_image_generations(
        json!({"prompt":"fixture","model":"unverified-flow-model"}),
    )
    .unwrap();
    let error = client
        .execute_json_passthrough(
            "test",
            crate::routing::candidate::ProviderExecutionMode::DirectHttp,
            &payload,
            &request,
            "unverified-flow-model",
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("flow_web_fresh_proof_unavailable")
    );
    assert!(!error.retryable);
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
}

#[tokio::test]
async fn flow_web_rejects_foreign_origin_before_loading_credentials() {
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "base_url":"https://flow.google.com.evil.invalid", "adapter":"flow_web_compatible", "api_key":""
    })).unwrap();
    let error = read_credits(&rquest::Client::new(), &payload)
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("flow_web_origin_invalid"));
}

#[test]
fn flow_web_preserves_ordinary_cookie_security_flags_on_https_only_transport() {
    let mut material = bundle();
    let mut ordinary = material["cookies"][0].clone();
    ordinary["name"] = json!("SID");
    ordinary["value"] = json!("synthetic-ordinary");
    ordinary["secure"] = json!(false);
    material["cookies"].as_array_mut().unwrap().push(ordinary);
    let mut jar =
        session::FlowCookies::from_bundle(&serde_json::to_vec(&material).unwrap()).unwrap();
    let mut headers = HeaderMap::new();
    jar.apply("https://flow.google.com/projects", &mut headers)
        .unwrap();
    assert!(headers[COOKIE]
        .to_str()
        .unwrap()
        .contains("SID=synthetic-ordinary"));
    assert!(jar
        .apply("http://flow.google.com/projects", &mut headers)
        .is_err());
    let mut updates = HeaderMap::new();
    updates.insert(
        SET_COOKIE,
        HeaderValue::from_static("SID=rotated; Domain=google.com; Path=/; HttpOnly"),
    );
    jar.receive("https://flow.google.com/projects", &updates)
        .unwrap();
    let snapshot: Value = serde_json::from_slice(&jar.bundle_bytes().unwrap()).unwrap();
    let sid = snapshot["cookies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "SID")
        .unwrap();
    assert_eq!(sid["secure"], false);
    assert_eq!(sid["value"], "rotated");
}
