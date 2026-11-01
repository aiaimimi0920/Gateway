use super::*;

fn source() -> Value {
    json!({"schemaVersion":1,"provider":"flow-web","authUser":"0","cookies":[{
        "name":"__Secure-1PSID","value":"fixture-private","domain":".google.com","path":"/",
        "expires":-1,"secure":true,"httpOnly":true,"sameSite":"Lax"
    }]})
}

#[test]
fn flow_web_json_is_independent_and_does_not_claim_generation_models() {
    let material = parse(&source().to_string(), GeminiAuthFamily::FlowWeb).unwrap();
    let input = CreateGeminiAuthSessionInput {
        target_family: GeminiAuthFamily::FlowWeb,
        provider_id: "renamed-flow".into(),
        account_label: Some("Flow account".into()),
    };
    let (draft, object) = draft(material, &input, "fixture").unwrap();
    assert_eq!(draft.provider_id, "renamed-flow");
    assert_eq!(draft.credential["supported_models"], json!([]));
    assert_eq!(
        draft.credential["model_selection"],
        json!({"fetched":[],"manual":[]})
    );
    assert_eq!(draft.credential["scheduled_probe_enabled"], false);
    assert!(!draft.credential.to_string().contains("fixture-private"));
    assert!(draft.secret_edits.is_empty());
    let (key, bytes) = object.unwrap();
    assert!(key.starts_with("credential-runtime/flow-web/"));
    let bundle: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(bundle["provider"], "flow-web");
    assert_eq!(bundle["authUser"], "0");
    assert!(bundle.get("origins").is_none());
}

#[test]
fn flow_web_rejects_other_google_products_and_secondary_account_claims() {
    assert!(parse(&source().to_string(), GeminiAuthFamily::GeminiCanvas).is_err());
    for (field, value) in [
        ("authUser", json!("1")),
        ("session", json!({"recaptchaToken":"fixture-private"})),
        ("origins", json!([])),
        ("provider", json!("flowmusic-web")),
    ] {
        let mut source = source();
        source[field] = value;
        let error = parse(&source.to_string(), GeminiAuthFamily::FlowWeb)
            .err()
            .unwrap();
        assert!(!error.message.contains("fixture-private"));
    }
    for domain in [
        "gemini.google.com",
        "aistudio.google.com",
        "google.com",
        ".evil.invalid",
    ] {
        let mut source = source();
        source["cookies"][0]["domain"] = json!(domain);
        assert!(parse(&source.to_string(), GeminiAuthFamily::FlowWeb).is_err());
    }
    let mut mixed = source();
    let mut host_only = mixed["cookies"][0].clone();
    host_only["domain"] = json!("google.com");
    host_only["name"] = json!("host_only");
    mixed["cookies"].as_array_mut().unwrap().push(host_only);
    assert!(parse(&mixed.to_string(), GeminiAuthFamily::FlowWeb).is_err());
    assert!(GeminiAuthFamily::FlowWeb.matches_provider_preset("flow-web"));
    assert!(!GeminiAuthFamily::FlowWeb.matches_provider_preset("producer"));
}

#[test]
fn flow_web_json_keeps_non_prefixed_cookie_flags_for_https_only_requests() {
    let mut source = source();
    let mut ordinary = source["cookies"][0].clone();
    ordinary["name"] = json!("SID");
    ordinary["secure"] = json!(false);
    source["cookies"].as_array_mut().unwrap().push(ordinary);
    assert!(parse(&source.to_string(), GeminiAuthFamily::FlowWeb).is_ok());
    source["cookies"][0]["secure"] = json!(false);
    assert!(parse(&source.to_string(), GeminiAuthFamily::FlowWeb).is_err());
}
