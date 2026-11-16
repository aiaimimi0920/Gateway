use super::*;
use serde_json::{json, Value};

fn reply(expiry: Value) -> Value {
    json!({"success":true,"data":{"accessToken":"synthetic-access","refreshToken":"synthetic-refresh","expiresAt":expiry}})
}
fn parse(value: Value, previous: Option<&str>) -> Result<TokenMaterial, OAuthError> {
    TokenMaterial::parse(value.to_string().as_bytes(), previous.map(str::to_owned))
}

#[test]
fn accio_material_accepts_numeric_seconds_milliseconds_and_explicit_timezone() {
    for expiry in [
        json!(1790000000),
        json!(1790000000000u64),
        json!("1790000000"),
        json!("1790000000000"),
        json!("2026-09-21T14:13:20Z"),
        json!("2026-09-21T22:13:20+08:00"),
    ] {
        let tokens = parse(reply(expiry), None).unwrap();
        assert_eq!(tokens.access_token(), "synthetic-access");
        assert_eq!(tokens.refresh_token(), "synthetic-refresh");
        assert_eq!(tokens.expires_at().unix_timestamp(), 1790000000);
        assert_eq!(tokens.expires_at().offset(), time::UtcOffset::UTC);
    }
}

#[test]
fn accio_decimal_expiry_keeps_absolute_precision_without_float_rounding_or_new_ttl() {
    for expiry in [
        "1790000000.123456789",
        "\"1790000000.123456789\"",
        "1790000000123.456789",
        "\"1790000000123.456789\"",
    ] {
        let wire = format!(
            r#"{{"success":true,"data":{{"accessToken":"a","refreshToken":"r","expiresAt":{expiry}}}}}"#
        );
        let tokens = TokenMaterial::parse(wire.as_bytes(), None).unwrap();
        assert_eq!(
            tokens.expires_at().unix_timestamp_nanos(),
            1790000000123456789
        );
    }
    let tokens = parse(reply(json!("2000-01-01T00:00:00Z")), None).unwrap();
    assert_eq!(
        tokens.expires_at().unix_timestamp(),
        946684800,
        "expired rotation material is retained, never granted a fresh lifetime"
    );
}

#[test]
fn accio_expiry_rejects_missing_zero_negative_ambiguous_unbounded_and_unsupported_formats() {
    for expiry in [
        Value::Null,
        json!(true),
        json!({}),
        json!(0),
        json!(-1),
        json!(""),
        json!("NaN"),
        json!("Infinity"),
        json!("2026-10-28 23:59:59"),
        json!("2026-10-28T23:59:59"),
        json!("2026/10/28"),
        json!("1790000000 "),
        json!("1e20"),
        json!("-1790000000"),
        json!("1.2.3"),
        json!("1."),
        json!(".1"),
        json!("1.1234567890"),
        json!("99999999999999999999999999999999"),
        json!("1000000000000"),
        json!("1970-01-01T00:00:00Z"),
    ] {
        assert_eq!(
            parse(reply(expiry.clone()), None).unwrap_err(),
            OAuthError::InvalidResponse,
            "{expiry}"
        );
    }
    let mut value = reply(json!(1790000000));
    value["data"].as_object_mut().unwrap().remove("expiresAt");
    assert_eq!(parse(value, None).unwrap_err(), OAuthError::InvalidResponse);
}

#[test]
fn accio_http_200_false_envelope_never_becomes_token_success_or_prose_diagnostic() {
    for value in [
        json!({"success":false,"code":"401","message":"private-access"}),
        json!({"success":false,"data":{"accessToken":"private","refreshToken":"private","expiresAt":1790000000}}),
    ] {
        let error = parse(value, None).unwrap_err();
        assert_eq!(error, OAuthError::Rejected);
        assert!(!error.to_string().contains("private"));
    }
    for value in [
        json!({"success":"true","data":{}}),
        json!({"data":{}}),
        json!({"success":true}),
        json!({"success":true,"data":null}),
        json!({"success":true,"data":[]}),
        json!({"success":true,"data":"private"}),
    ] {
        assert_eq!(parse(value, None).unwrap_err(), OAuthError::InvalidResponse);
    }
}

#[test]
fn accio_material_requires_both_nonempty_bounded_opaque_tokens() {
    for name in ["accessToken", "refreshToken"] {
        for field in [
            Value::Null,
            json!(""),
            json!(" "),
            json!("bad\nprivate"),
            json!({}),
            json!(true),
            json!("x".repeat(16_385)),
        ] {
            let mut value = reply(json!(1790000000));
            value["data"][name] = field;
            assert_eq!(parse(value, None).unwrap_err(), OAuthError::InvalidResponse);
        }
        let mut value = reply(json!(1790000000));
        value["data"].as_object_mut().unwrap().remove(name);
        assert_eq!(parse(value, None).unwrap_err(), OAuthError::InvalidResponse);
    }
}

#[test]
fn accio_response_rejects_duplicate_sensitive_fields_and_trailing_json() {
    for wire in [
        r#"{"success":false,"success":true,"data":{}}"#,
        r#"{"success":true,"data":{},"data":{}}"#,
        r#"{"success":true,"data":{"accessToken":"a","accessToken":"b","refreshToken":"r","expiresAt":1790000000}}"#,
        r#"{"success":true,"data":{"accessToken":"a","refreshToken":"r","refreshToken":"b","expiresAt":1790000000}}"#,
        r#"{"success":true,"data":{"accessToken":"a","refreshToken":"r","expiresAt":1790000000,"expiresAt":1790000060}}"#,
        r#"{"success":true,"data":{"accessToken":"a","refreshToken":"r","expiresAt":1790000000,"sgcookie":"a","sgcookie":"b"}}"#,
        r#"{"success":false} {}"#,
        "[]",
        "private non-json",
        "null",
    ] {
        assert_eq!(
            TokenMaterial::parse(wire.as_bytes(), None).unwrap_err(),
            OAuthError::InvalidResponse
        );
    }
}

#[test]
fn accio_cookie_rotates_or_retains_previous_secret_without_making_an_identity() {
    let mut value = reply(json!(1790000000));
    assert_eq!(
        parse(value.clone(), Some("old-cookie")).unwrap().sgcookie(),
        Some("old-cookie")
    );
    assert_eq!(parse(value.clone(), None).unwrap().sgcookie(), None);
    value["data"]["sgcookie"] = json!("new-cookie");
    let tokens = parse(value.clone(), Some("old-cookie")).unwrap();
    assert_eq!(tokens.sgcookie(), Some("new-cookie"));
    let debug = format!("{tokens:?}");
    for secret in [
        tokens.access_token(),
        tokens.refresh_token(),
        tokens.sgcookie().unwrap(),
    ] {
        assert!(!debug.contains(secret));
    }
    assert!(debug.contains("unverified"));
    for cookie in [
        json!(""),
        json!(" "),
        json!("bad\r\ncookie"),
        json!("x".repeat(16_385)),
        json!({}),
    ] {
        value["data"]["sgcookie"] = cookie;
        assert_eq!(
            parse(value.clone(), Some("old-cookie")).unwrap_err(),
            OAuthError::InvalidResponse
        );
    }
}
