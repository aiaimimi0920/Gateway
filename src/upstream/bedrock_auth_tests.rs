//! Synthetic credentials only; explicit authentication must fail closed before IO.
use super::*;
use crate::upstream::headers::build_upstream_headers;
use serde_json::json;

fn payload(mode: Option<&str>, extra: serde_json::Value) -> ProviderAccountPayload {
    serde_json::from_value(json!({
        "adapter":"bedrock_converse_compatible",
        "base_url":"https://bedrock-runtime.us-east-1.amazonaws.com",
        "api_key":"", "auth_mode":mode, "extra_body":extra
    }))
    .unwrap()
}

fn plan(base: &str) -> RequestPlan {
    RequestPlan {
        method: rquest::Method::POST,
        url: format!("{base}/model/fixture/converse-stream"),
        query: vec![],
        body: Some(json!({"messages":[]})),
        response_kind: crate::protocol::canonical::EndpointKind::ChatCompletions,
    }
}

fn iam() -> serde_json::Value {
    json!({"awsAccessKeyId":"AKIAFIXTURE", "awsSecretAccessKey":"secret-fixture", "awsRegion":"us-east-1"})
}

#[test]
fn bedrock_bearer_key_is_not_a_raw_authorization_header() {
    let mut p = payload(Some("bedrock_bearer"), json!({"awsRegion":"us-east-1"}));
    p.api_key = "bedrock-key-fixture".into();
    p.auth_header_name = Some("authorization".into());
    let headers =
        sign_bedrock_runtime_headers_if_needed(&p, &plan(&p.base_url), build_upstream_headers(&p))
            .unwrap();
    assert_eq!(headers["authorization"], "Bearer bedrock-key-fixture");
}

#[test]
fn bedrock_incomplete_or_conflicting_modes_fail_closed() {
    for (mode, extra, key) in [
        ("aws_sigv4", json!({}), ""),
        ("aws_sigv4", json!({"awsAccessKeyId":"AKIAFIXTURE"}), ""),
        ("aws_sigv4_temporary", iam(), ""),
        ("bedrock_bearer", iam(), "key-fixture"),
        ("aws_sigv4", iam(), "key-fixture"),
        ("unsupported-mode", json!({}), "key-fixture"),
    ] {
        let mut p = payload(Some(mode), extra);
        p.api_key = key.into();
        let error =
            sign_bedrock_runtime_headers_if_needed(&p, &plan(&p.base_url), HeaderMap::new()).err();
        assert!(error.is_some(), "accepted invalid mode {mode}");
        let message = error.unwrap().to_string();
        assert!(!message.contains("secret-fixture") && !message.contains("key-fixture"));
    }
}

#[test]
fn bedrock_explicit_auth_rejects_unbound_targets_and_signing_regions() {
    for base in [
        "https://bedrock-runtime.us-east-1.amazonaws.com.evil.example",
        "https://bedrock-runtime.us-west-2.amazonaws.com",
        "http://bedrock-runtime.us-east-1.amazonaws.com",
        "https://user@bedrock-runtime.us-east-1.amazonaws.com",
        "https://bedrock-runtime.us-east-1.amazonaws.com/prefix",
    ] {
        let p = payload(Some("aws_sigv4"), iam());
        assert!(
            sign_bedrock_runtime_headers_if_needed(&p, &plan(base), HeaderMap::new()).is_err(),
            "accepted {base}"
        );
    }
    let p = payload(Some("aws_sigv4"), iam());
    for suffix in ["?leak=1", "#fragment"] {
        let mut request = plan(&p.base_url);
        request.url.push_str(suffix);
        assert!(sign_bedrock_runtime_headers_if_needed(&p, &request, HeaderMap::new()).is_err());
    }
}

#[test]
fn bedrock_long_term_and_temporary_iam_are_distinct_material_modes() {
    for mode in [None, Some("aws_sigv4"), Some("aws_sigv4_temporary")] {
        let mut fields = iam();
        if mode == Some("aws_sigv4_temporary") {
            fields["awsSessionToken"] = json!("session-fixture");
        }
        let p = payload(mode, fields);
        let headers =
            sign_bedrock_runtime_headers_if_needed(&p, &plan(&p.base_url), HeaderMap::new())
                .unwrap();
        assert!(headers["authorization"]
            .to_str()
            .unwrap()
            .starts_with("AWS4-HMAC-SHA256 "));
        assert_eq!(
            headers.contains_key("x-amz-security-token"),
            mode == Some("aws_sigv4_temporary")
        );
    }
}

#[test]
fn bedrock_auth_finalization_drops_stale_signing_identity() {
    for mode in ["bedrock_bearer", "aws_sigv4"] {
        let mut p = payload(
            Some(mode),
            if mode == "aws_sigv4" {
                iam()
            } else {
                json!({"awsRegion":"us-east-1"})
            },
        );
        if mode == "bedrock_bearer" {
            p.api_key = "key-fixture".into();
        }
        let mut headers = HeaderMap::new();
        for name in [
            "authorization",
            "x-amz-date",
            "x-amz-security-token",
            "x-amz-content-sha256",
            "host",
            "content-length",
        ] {
            insert_header_map_value(&mut headers, name, "stale-fixture");
        }
        let headers =
            sign_bedrock_runtime_headers_if_needed(&p, &plan(&p.base_url), headers).unwrap();
        assert!(!headers.values().any(|value| value == "stale-fixture"));
        assert!(!headers.contains_key("x-amz-security-token"));
    }
}
