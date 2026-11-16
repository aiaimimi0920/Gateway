//! Legacy exports must retain complete opaque grant material without making new authority.
#![cfg(feature = "line-accio-web-reverse-api")]
use super::{super::normalization::normalize_import_payload, build_test_provider_account};
use crate::{
    db,
    error::GatewayError,
    protocol::accio_oauth::{GATEWAY, REFRESH_ENDPOINT},
};
use serde_json::{json, Value};

fn provider() -> db::GatewayProviderAccountView {
    build_test_provider_account(
        "Accio",
        "accio_compatible",
        "accio",
        "openai_responses",
        GATEWAY,
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    )
}
fn raw() -> Value {
    json!({"accessToken":"synthetic-access", "refreshToken":"synthetic-refresh", "sgcookie":"synthetic sg opaque",
        "refreshClientId":"explicit-client", "refreshEndpoint":REFRESH_ENDPOINT,
        "expiresAt":"2026-01-01T08:00:00.123456789+08:00", "utdid":"synthetic-device", "id":"account-1"})
}
fn normalize(raw: Value) -> Result<Value, GatewayError> {
    normalize_import_payload("accio", &provider(), raw, None)
}
fn private_error(error: GatewayError) {
    assert_eq!(error.code.as_deref(), Some("accio_import_invalid_material"));
    for value in [
        "synthetic-access",
        "synthetic-refresh",
        "synthetic sg opaque",
        "explicit-client",
        "private",
    ] {
        assert!(
            !error.to_string().contains(value),
            "unsafe import diagnostic"
        );
    }
}

#[test]
fn accio_import_retains_complete_grant_and_removes_body_authentication_copies() {
    for nested in [false, true] {
        let mut input = raw();
        if nested {
            let extra = input.as_object_mut().unwrap();
            let fields = [
                "refreshToken",
                "sgcookie",
                "refreshClientId",
                "refreshEndpoint",
                "expiresAt",
            ];
            let mut material = serde_json::Map::new();
            for field in fields {
                material.insert(field.into(), extra.remove(field).unwrap());
            }
            extra.insert("extraBody".into(), Value::Object(material));
        }
        let canonical = normalize(input.clone()).unwrap();
        assert_eq!(canonical["apiKey"], "synthetic-access");
        assert_eq!(canonical["refreshToken"], "synthetic-refresh");
        assert_eq!(canonical["accioSgcookie"], "synthetic sg opaque");
        assert_eq!(canonical["refreshClientId"], "explicit-client");
        assert_eq!(canonical["refreshEndpoint"], REFRESH_ENDPOINT);
        assert_eq!(canonical["expiresAt"], "2026-01-01T00:00:00.123456789Z");
        assert_eq!(canonical["rawSource"], input);
        assert_eq!(
            canonical["credentialMaterialKey"],
            "accio-account:account-1"
        );
        for key in ["token", "accessToken", "utdid", "refreshToken", "sgcookie"] {
            assert!(canonical["extraBody"].get(key).is_none(), "duplicate {key}");
        }
        assert!(!canonical["headers"]
            .to_string()
            .contains("synthetic sg opaque"));
    }
}

#[test]
fn accio_import_preserves_exact_absolute_seconds_milliseconds_and_expired_dates() {
    for (input, expected) in [
        (json!(1767225600u64), "2026-01-01T00:00:00Z"),
        (json!(1767225600123u64), "2026-01-01T00:00:00.123Z"),
        (
            json!("1767225600.123456789"),
            "2026-01-01T00:00:00.123456789Z",
        ),
        (
            json!("1767225600123.456789"),
            "2026-01-01T00:00:00.123456789Z",
        ),
        (json!("2000-01-01T00:00:00Z"), "2000-01-01T00:00:00Z"),
    ] {
        let mut source = raw();
        source["expiresAt"] = input;
        for _ in 0..2 {
            assert_eq!(normalize(source.clone()).unwrap()["expiresAt"], expected);
        }
    }
}

#[test]
fn accio_import_does_not_invent_client_expiry_or_refresh_authority() {
    let source = json!({"accessToken":"synthetic-access", "utdid":"synthetic-device"});
    let canonical = normalize(source).unwrap();
    for key in [
        "refreshToken",
        "refreshClientId",
        "refreshEndpoint",
        "expiresAt",
        "accioSgcookie",
    ] {
        assert!(canonical.get(key).is_none(), "invented {key}");
    }
    let canonical = normalize(json!({"accessToken":"synthetic-access", "utdid":"synthetic-device", "refreshToken":"synthetic-refresh"})).unwrap();
    assert_eq!(canonical["refreshToken"], "synthetic-refresh");
    assert!(
        canonical.get("refreshClientId").is_none() && canonical.get("refreshEndpoint").is_none()
    );
    let mut source = raw();
    source.as_object_mut().unwrap().remove("refreshEndpoint");
    assert_eq!(
        normalize(source).unwrap()["refreshEndpoint"],
        REFRESH_ENDPOINT
    );
}

#[test]
fn accio_import_rejects_bad_or_ambiguous_expiry_without_extending_a_lifetime() {
    for invalid in [
        json!(0),
        json!(-1),
        json!(1767225600.5),
        json!([]),
        json!({}),
        json!(""),
        json!("2026-01-01T00:00:00"),
        json!("private not a date"),
    ] {
        let mut source = raw();
        source["expiresAt"] = invalid;
        private_error(normalize(source).unwrap_err());
    }
    let mut source = raw();
    source["expires_at"] = json!("2099-01-01T00:00:00Z");
    private_error(normalize(source).unwrap_err());
}

#[test]
fn accio_import_rejects_bad_types_secret_conflicts_and_foreign_refresh_targets() {
    for key in [
        "refreshToken",
        "sgcookie",
        "refreshClientId",
        "refreshEndpoint",
    ] {
        for invalid in [json!(5), json!([]), json!(""), json!("private\r\nvalue")] {
            let mut source = raw();
            source[key] = invalid;
            private_error(normalize(source).unwrap_err());
        }
    }
    for extra in [
        json!({"refresh_token":"private-other"}),
        json!({"accio_sgcookie":"private-other"}),
        json!({"refresh_client_id":"private-other"}),
    ] {
        let mut source = raw();
        source["extra_body"] = extra;
        private_error(normalize(source).unwrap_err());
    }
    let mut source = raw();
    source["refreshEndpoint"] = json!("https://foreign.invalid/private");
    private_error(normalize(source).unwrap_err());
}

#[test]
fn accio_import_rejects_effective_foreign_audience_and_invalid_device() {
    for base in [
        "https://foreign.invalid",
        "http://phoenix-gw.alibaba.com",
        "https://user@phoenix-gw.alibaba.com",
        "https://phoenix-gw.alibaba.com?private=1",
    ] {
        let mut owner = provider();
        owner.payload["baseUrl"] = json!(base);
        private_error(normalize_import_payload("accio", &owner, raw(), None).unwrap_err());
    }
    let mut source = raw();
    source["utdid"] = json!("private\r\ndevice");
    private_error(normalize(source).unwrap_err());
}

#[test]
fn accio_import_canonical_material_survives_an_export_import_roundtrip() {
    let first = normalize(raw()).unwrap();
    let second = normalize(first.clone()).unwrap();
    for key in [
        "apiKey",
        "refreshToken",
        "accioSgcookie",
        "refreshEndpoint",
        "refreshClientId",
        "expiresAt",
        "rawSource",
    ] {
        assert_eq!(first[key], second[key], "roundtrip changed {key}");
    }
}

#[test]
fn accio_import_rejects_disagreeing_or_malformed_access_token_aliases() {
    for change in [
        json!({"apiKey":"private-other"}),
        json!({"extraBody":{"token":"private-other"}}),
        json!({"access_token":5}),
        json!({"accessToken":"private\r\ntoken"}),
    ] {
        let mut source = raw();
        source
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        private_error(normalize(source).unwrap_err());
    }
}

#[test]
fn accio_import_ordinary_native_planner_binds_canonical_auth_without_forwarding_refresh_sg_or_source(
) {
    let mut canonical = normalize(raw()).unwrap();
    canonical["adapter"] = json!("accio_compatible");
    let payload = serde_json::from_value(canonical).unwrap();
    let request =
        crate::protocol::openai::normalize_chat_completions(json!({"model":"configured-model",
        "messages":[{"role":"user","content":"synthetic import wire"}]}))
        .unwrap();
    let plan =
        crate::upstream::accio::build_request_plan(&payload, &request, "configured-model", true)
            .unwrap();
    assert_eq!(plan.url, format!("{GATEWAY}/api/adk/llm/generateContent"));
    let body = plan.body.unwrap();
    assert_eq!(body["token"], "synthetic-access");
    assert_eq!(body["utdid"], "synthetic-device");
    assert_eq!(body["model"], "configured-model");
    for private in [
        "synthetic-refresh",
        "synthetic sg opaque",
        "explicit-client",
        "rawSource",
        "refreshEndpoint",
    ] {
        assert!(
            !body.to_string().contains(private),
            "business wire leaked refresh material"
        );
    }
}
