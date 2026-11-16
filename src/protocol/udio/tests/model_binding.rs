//! Model identity is routing-owned, not a second public body override.
use super::*;

#[test]
fn udio_model_binding_uses_resolved_identity_not_public_alias_or_legacy_default() {
    let req = make_request(json!({"model":"public-alias","prompt":"fixture"}));
    for model in ["udio-fixture-native", "another-configured-native"] {
        let body = build_generate_request(&req, model).unwrap();
        assert_eq!(body["gen_params"]["model_type"], model);
    }
}

#[test]
fn udio_model_binding_keeps_legacy_operation_alias_explicit() {
    let body = build_generate_request(
        &make_request(json!({"prompt":"fixture"})),
        UDIO_DEFAULT_MODEL,
    )
    .unwrap();
    assert_eq!(body["gen_params"]["model_type"], UDIO_DEFAULT_MODEL_TYPE);
}

#[test]
fn udio_model_binding_accepts_equal_explicit_aliases_without_changing_identity() {
    let model = "udio-fixture-native";
    let req = make_request(
        json!({"prompt":"fixture","model_type":model,"modelType":model,
        "upstream_model_type":model,"upstreamModelType":model,"gen_params":{"model_type":model}}),
    );
    assert_eq!(
        build_generate_request(&req, model).unwrap()["gen_params"]["model_type"],
        model
    );
}

#[test]
fn udio_model_binding_rejects_every_conflicting_or_malformed_override() {
    for nested in [false, true] {
        for key in [
            "model_type",
            "modelType",
            "upstream_model_type",
            "upstreamModelType",
        ] {
            for value in [
                json!("foreign-private-model"),
                Value::Null,
                json!(false),
                json!(17),
                json!(""),
                json!(" udio-fixture-native "),
            ] {
                let mut body =
                    json!({"prompt":"fixture","model_type":"udio-fixture-native","gen_params":{}});
                if nested {
                    body["gen_params"][key] = value;
                } else {
                    body[key] = value;
                }
                let error =
                    build_generate_request(&make_request(body), "udio-fixture-native").unwrap_err();
                assert_eq!(
                    error.code.as_deref(),
                    Some("udio_model_binding_invalid"),
                    "{key}"
                );
                assert_eq!(error.http_status, Some(400));
                assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
                assert!(!error.retryable);
                assert!(matches!(
                    error.fallback_hint,
                    crate::error::FallbackHint::Abort { .. }
                ));
                assert!(!error.message.contains("foreign-private-model"));
            }
        }
    }
}

#[test]
fn udio_model_binding_rejects_invalid_resolved_identity_and_gen_params() {
    for model in [
        "",
        " ",
        " trailing",
        "with space",
        "bad\nmodel",
        &"x".repeat(129),
    ] {
        assert!(build_generate_request(&make_request(json!({"prompt":"fixture"})), model).is_err());
    }
    for value in [
        Value::Null,
        json!([]),
        json!("private-parameters"),
        json!(false),
    ] {
        let error = build_generate_request(
            &make_request(json!({"prompt":"fixture","gen_params":value})),
            "udio-fixture-native",
        )
        .unwrap_err();
        assert_eq!(error.code.as_deref(), Some("udio_model_binding_invalid"));
    }
}
