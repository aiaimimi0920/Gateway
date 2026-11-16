//! Typed Kiro contexts bind the resolved business target and reject inherited legacy credentials.
use super::*;

#[test]
fn kiro_typed_config_rejects_nested_registration_and_archive_copies() {
    for name in ["kiro_oauth", "grokOauth", "accio_sgcookie", "rawSource"] {
        let mut doc = document(context());
        doc.providers[0].credentials[0].extra_body.insert(
            name.into(),
            json!({"client_secret":"private-copy", "refreshToken":"private-refresh"}),
        );
        assert!(
            RouteConfigStore::from_document(doc).is_err(),
            "accepted nested auth copy {name}"
        );
    }
}

#[test]
fn kiro_typed_access_and_refresh_tokens_are_bounded_atomic_material() {
    for token in [" ".into(), "bad\ntoken".into(), "x".repeat(32_769)] {
        for refresh in [false, true] {
            let mut doc = document(context());
            let credential = &mut doc.providers[0].credentials[0];
            if refresh {
                credential.refresh_token = Some(token.clone());
            } else {
                credential.api_key = Some(token.clone());
            }
            assert!(RouteConfigStore::from_document(doc).is_err());
        }
    }
    let mut missing = document(context());
    missing.providers[0].api_key = "provider-access".into();
    missing.providers[0].credentials[0].api_key = None;
    assert!(RouteConfigStore::from_document(missing).is_err());
}

#[test]
fn kiro_context_rejects_auth_header_overrides_at_provider_and_credential_levels() {
    for name in [
        "Authorization",
        "Proxy-Authorization",
        "Cookie",
        "X-Api-Key",
        "api-key",
        "x-goog-api-key",
        "Host",
        "User-Agent",
        "X-Amz-User-Agent",
    ] {
        for provider_level in [false, true] {
            let mut doc = document(context());
            if provider_level {
                doc.providers[0]
                    .headers
                    .insert(name.into(), "legacy-secret".into());
            } else {
                doc.providers[0].credentials[0]
                    .headers
                    .insert(name.into(), "legacy-secret".into());
            }
            assert!(
                RouteConfigStore::from_document(doc).is_err(),
                "accepted legacy header {name}"
            );
        }
    }
}

#[test]
fn kiro_context_rejects_foreign_target_generic_session_and_legacy_body_auth_copies() {
    for extra in [
        "kiroRefreshToken",
        "refresh_token",
        "Kiro-Access-Token",
        "clientSecret",
        "authMethod",
        "authRegion",
        "apiRegion",
        "apiKey",
        "authToken",
        "authorization",
        "cookie",
        "token",
    ] {
        for provider_level in [false, true] {
            let mut doc = document(context());
            let map = if provider_level {
                &mut doc.providers[0].extra_body
            } else {
                &mut doc.providers[0].credentials[0].extra_body
            };
            map.insert(extra.into(), json!("legacy-secret"));
            assert!(
                RouteConfigStore::from_document(doc).is_err(),
                "accepted legacy field {extra}"
            );
        }
    }
    for change in [
        json!({"base_url":"https://q.us-east-1.amazonaws.com.evil.invalid"}),
        json!({"base_url":"http://q.us-east-1.amazonaws.com"}),
        json!({"base_url":"https://q.cn-north-1.amazonaws.com.cn"}),
        json!({"base_url":"https://q.us-east-1.amazonaws.com:444"}),
        json!({"base_url":"https://q.us-east-1.amazonaws.com/other"}),
        json!({"base_url":"https://oidc.us-east-1.amazonaws.com"}),
        json!({"preset":"xai-api","base_url":"https://api.x.ai/v1"}),
        json!({"session_auth":{"transport":"cookie"}}),
        json!({"responses_path":"/other"}),
        json!({"auth_token":"legacy-secret"}),
        json!({"keepalive":{"serviceUrl":"https://foreign.invalid"}}),
        json!({"execution_mode":"browser_backed"}),
    ] {
        let mut value = serde_json::to_value(document(context())).unwrap();
        let provider = value["providers"][0].as_object_mut().unwrap();
        for (key, value) in change.as_object().unwrap() {
            provider.insert(key.clone(), value.clone());
        }
        assert!(
            RouteConfigStore::from_document(serde_json::from_value(value).unwrap()).is_err(),
            "accepted {change}"
        );
    }
}
