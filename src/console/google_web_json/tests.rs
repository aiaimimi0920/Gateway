use super::*;

fn cookie(name: &str) -> Value {
    json!({"name":name,"value":"fixture-private-cookie","domain":".google.com","path":"/","expires":-1,"secure":true,"httpOnly":true,"sameSite":"Lax"})
}
fn material(family: GeminiAuthFamily) -> Value {
    let names = match family {
        GeminiAuthFamily::GeminiWeb => vec!["__Secure-1PSID", "__Secure-1PSIDTS"],
        GeminiAuthFamily::GeminiCanvas => vec!["SAPISID"],
        _ => vec!["SAPISID", "__Secure-1PAPISID", "__Secure-3PAPISID"],
    };
    json!({"schemaVersion":1,"provider":provider_name(family).unwrap(),"authUser":if family == GeminiAuthFamily::GeminiWeb {"0"} else {"1"},"cookies":names.into_iter().map(cookie).collect::<Vec<_>>()})
}
fn rejected(value: &Value, family: GeminiAuthFamily) {
    let error = parse(&value.to_string(), family)
        .err()
        .expect("must reject");
    assert!(!error.message.contains("fixture-private-cookie"));
}

#[test]
fn all_three_formats_build_exact_target_drafts_without_claiming_online_verification() {
    for family in [
        GeminiAuthFamily::GeminiWeb,
        GeminiAuthFamily::GeminiCanvas,
        GeminiAuthFamily::AistudioWeb,
    ] {
        let parsed = parse(&material(family).to_string(), family).unwrap();
        let input = CreateGeminiAuthSessionInput {
            target_family: family,
            provider_id: "renamed-pool".into(),
            account_label: Some("  My account  ".into()),
        };
        let (draft, object) = draft(parsed, &input, "fixture-id").unwrap();
        assert_eq!(draft.provider_id, "renamed-pool");
        assert_eq!(draft.credential["account_name"], "My account");
        assert_eq!(draft.credential["scheduled_probe_enabled"], false);
        assert_eq!(draft.credential["model_selection"]["fetched"], json!([]));
        assert!(draft.credential.get("verified_models").is_none());
        assert!(!draft
            .credential
            .to_string()
            .contains("fixture-private-cookie"));
        if family == GeminiAuthFamily::GeminiWeb {
            assert!(object.is_none());
            assert_eq!(draft.secret_edits.len(), 3);
            assert_eq!(draft.credential["extra_body"]["authUser"], "0");
        } else {
            assert!(draft.secret_edits.is_empty());
            let (key, bytes) = object.unwrap();
            assert!(key.starts_with("credential-runtime/"));
            let stored: Value = serde_json::from_slice(&bytes).unwrap();
            assert!(stored.get("origins").is_none());
            assert_eq!(stored["cookies"][0]["domain"], ".google.com");
            assert_eq!(stored["cookies"][0]["expires"], -1.0);
            if family == GeminiAuthFamily::AistudioWeb {
                assert_eq!(stored["authUser"], "1");
            }
        }
    }
}

#[test]
fn rejects_wrong_provider_browser_state_oauth_paths_and_unknown_cookie_fields() {
    let family = GeminiAuthFamily::AistudioWeb;
    for (key, value) in [
        ("provider", json!("gemini-web")),
        ("schemaVersion", json!(2)),
        ("authUser", json!("0/../../")),
        ("origins", json!([])),
        ("localStorage", json!({})),
        ("refreshToken", json!("fixture-private-cookie")),
        ("runtimeStateObjectKey", json!("C:/private")),
        ("session", json!({"endpointPath":"https://evil.invalid"})),
    ] {
        let mut data = material(family);
        data[key] = value;
        rejected(&data, family);
    }
    for (key, value) in [
        ("domain", json!("evil.invalid")),
        ("value", json!("fixture-private-cookie\r\nInjected:true")),
        ("partitionKey", json!("https://evil.invalid")),
        ("expires", json!(253402300800_u64)),
        ("path", json!("relative")),
        ("secure", json!(false)),
    ] {
        let mut data = material(family);
        data["cookies"][1][key] = value;
        rejected(&data, family);
    }
}

#[test]
fn required_auth_cookies_must_be_current_and_reach_the_selected_product() {
    for family in [
        GeminiAuthFamily::GeminiWeb,
        GeminiAuthFamily::GeminiCanvas,
        GeminiAuthFamily::AistudioWeb,
    ] {
        for field in ["expires", "domain", "path", "value"] {
            let mut data = material(family);
            for item in data["cookies"].as_array_mut().unwrap() {
                item[field] = match field {
                    "expires" => json!(1),
                    "domain" => json!("google.com"),
                    "path" => json!("/elsewhere"),
                    _ => json!(""),
                };
            }
            rejected(&data, family);
        }
    }
}

#[test]
fn rejects_limits_duplicates_and_ambiguous_web_cookie_scopes() {
    let family = GeminiAuthFamily::GeminiWeb;
    assert!(parse(&"x".repeat(MAX_BYTES + 1), family).is_err());
    let mut data = material(family);
    data["cookies"] = json!(vec![cookie("__Secure-1PSID"); 65]);
    rejected(&data, family);
    let mut data = material(family);
    data["cookies"][1] = data["cookies"][0].clone();
    rejected(&data, family);
    let mut data = material(family);
    data["cookies"][0]["value"] = json!("x".repeat(8193));
    rejected(&data, family);
    for family in [GeminiAuthFamily::GeminiWeb, GeminiAuthFamily::GeminiCanvas] {
        let mut data = material(family);
        data["cookies"][0]["value"] = json!("x".repeat(8192));
        rejected(&data, family);
    }
    let mut data = material(family);
    data["cookies"][1] = data["cookies"][0].clone();
    data["cookies"][1]["domain"] = json!("gemini.google.com");
    let parsed = parse(&data.to_string(), family).unwrap();
    assert!(cookies::web_header(&parsed.cookies).is_err());
    let mut data = material(family);
    for i in 0..5 {
        let mut c = cookie(&format!("extra{i}"));
        c["value"] = json!("x".repeat(7000));
        data["cookies"].as_array_mut().unwrap().push(c);
    }
    rejected(&data, family);
}

#[test]
fn only_explicit_web_metadata_is_allowed_and_no_experimental_family_is_enabled() {
    let family = GeminiAuthFamily::GeminiWeb;
    let mut data = material(family);
    data["session"] = json!({"language":"en","modelHeader":"[1,2,3]"});
    assert!(parse(&data.to_string(), family).is_ok());
    for key in [
        "endpointPath",
        "accessToken",
        "sessionId",
        "headers",
        "executablePath",
    ] {
        data["session"] = json!({key:"fixture-private-cookie"});
        rejected(&data, family);
    }
    data["session"] = json!({});
    data["authUser"] = json!("1");
    rejected(&data, family);
    assert!(parse(
        &material(family).to_string(),
        GeminiAuthFamily::GeminiCanvasChat
    )
    .is_err());
    assert!(parse(
        &material(family).to_string(),
        GeminiAuthFamily::GeminiBusiness
    )
    .is_err());
}
