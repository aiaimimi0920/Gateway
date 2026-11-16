//! Explicit modes bind final destinations; legacy readers remain separate.
use super::*;
use serde_json::json;

fn payload(mode: &str) -> ProviderAccountPayload {
    serde_json::from_value(json!({
        "adapter":"bedrock_converse_compatible", "api_key":"key-fixture",
        "base_url":"https://bedrock-runtime.us-east-1.amazonaws.com", "auth_mode":mode,
        "extra_body":{"awsRegion":"us-east-1"}
    }))
    .unwrap()
}

fn plan(p: &ProviderAccountPayload, model: &str) -> RequestPlan {
    RequestPlan {
        method: rquest::Method::POST,
        url: format!(
            "{}{}",
            p.base_url,
            crate::protocol::bedrock_converse::default_path(model, true)
        ),
        query: vec![],
        body: Some(json!({"messages":[]})),
        response_kind: crate::protocol::canonical::EndpointKind::ChatCompletions,
    }
}

#[test]
fn bedrock_arn_is_one_encoded_label_and_cannot_escape_the_model_path() {
    let p = payload("bedrock_bearer");
    let model = "arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.fixture-v1:0";
    let request = plan(&p, model);
    assert!(request.url.ends_with("arn%3Aaws%3Abedrock%3Aus-east-1%3A123456789012%3Ainference-profile%2Fus.fixture-v1%3A0/converse-stream"));
    sign_bedrock_runtime_headers_if_needed(&p, &request, HeaderMap::new()).unwrap();
    for model in [
        "..",
        "../escape",
        "fixture?leak=1",
        "fixture#leak",
        "fixture\\escape",
        "fixture%2Fescape",
    ] {
        assert!(
            crate::upstream::bedrock_auth::validate_model_id(model).is_err(),
            "accepted {model}"
        );
    }
}

#[test]
fn bedrock_only_verified_public_runtime_hosts_are_enabled_for_explicit_modes() {
    for (base, region, allowed) in [
        (
            "https://bedrock-runtime.ap-northeast-1.amazonaws.com",
            "ap-northeast-1",
            true,
        ),
        (
            "https://bedrock-runtime-fips.us-gov-west-1.amazonaws.com",
            "us-gov-west-1",
            true,
        ),
        (
            "https://bedrock-runtime-fips.ap-northeast-1.amazonaws.com",
            "ap-northeast-1",
            false,
        ),
        (
            "https://bedrock.us-east-1.amazonaws.com",
            "us-east-1",
            false,
        ),
        (
            "https://bedrock-runtime.cn-north-1.amazonaws.com.cn",
            "cn-north-1",
            false,
        ),
        (
            "https://vpce-fixture.bedrock-runtime.us-east-1.vpce.amazonaws.com",
            "us-east-1",
            false,
        ),
        (
            "https://bedrock-runtime.us-east-1.amazonaws.com:8443",
            "us-east-1",
            false,
        ),
    ] {
        let mut p = payload("bedrock_bearer");
        p.base_url = base.into();
        p.extra_body
            .as_mut()
            .unwrap()
            .insert("awsRegion".into(), json!(region));
        assert_eq!(
            sign_bedrock_runtime_headers_if_needed(&p, &plan(&p, "fixture"), HeaderMap::new())
                .is_ok(),
            allowed,
            "{base}"
        );
    }
}

#[test]
fn bedrock_alias_conflicts_invalid_fields_and_missing_region_fail_without_secret_diagnostics() {
    for fields in [
        json!({}),
        json!({"awsRegion":null}),
        json!({"awsRegion":"us-east-1", "aws_region":"us-west-2"}),
    ] {
        let mut p = payload("bedrock_bearer");
        p.extra_body = serde_json::from_value(fields).unwrap();
        assert!(
            sign_bedrock_runtime_headers_if_needed(&p, &plan(&p, "fixture"), HeaderMap::new())
                .is_err()
        );
    }
    for key in ["", "Bearer key-fixture", "key\r\nleak", " key-fixture"] {
        let mut p = payload("bedrock_bearer");
        p.api_key = key.into();
        assert!(
            sign_bedrock_runtime_headers_if_needed(&p, &plan(&p, "fixture"), HeaderMap::new())
                .is_err()
        );
    }
}

#[test]
fn bedrock_legacy_no_mode_and_custom_header_are_not_reinterpreted_as_new_material_modes() {
    for mode in [None, Some("bearer")] {
        let mut p = payload("bearer");
        p.auth_mode = mode.map(str::to_owned);
        p.auth_header_name = Some("x-legacy-token".into());
        p.base_url = "https://custom.example".into();
        let headers = crate::upstream::headers::build_upstream_headers(&p);
        let headers =
            sign_bedrock_runtime_headers_if_needed(&p, &plan(&p, "fixture"), headers).unwrap();
        assert_eq!(headers["x-legacy-token"], "key-fixture");
    }
}

#[test]
fn bedrock_explicit_material_cannot_change_adapter_or_model_path() {
    let request = crate::protocol::responses::normalize_responses(
        json!({"model":"fixture", "input":"hello"}),
    )
    .unwrap();
    let mut p = payload("bedrock_bearer");
    p.chat_completions_path = Some("/model/different/converse-stream".into());
    assert!(crate::upstream::client::UpstreamClient::build_request_plan(
        &p, &request, "fixture", true
    )
    .is_err());
    p.chat_completions_path = None;
    p.call_flow =
        serde_json::from_value(json!({"call_adapter":"openai", "special_adapter":"none"})).unwrap();
    assert!(crate::upstream::client::UpstreamClient::build_request_plan(
        &p, &request, "fixture", true
    )
    .is_err());
    p.adapter = "openai_compatible".into();
    assert!(
        sign_bedrock_runtime_headers_if_needed(&p, &plan(&p, "fixture"), HeaderMap::new()).is_err()
    );
    assert!(crate::protocol::call_flow::prepare(&p).is_err());
    p.call_flow = None;
    assert!(crate::protocol::call_flow::prepare(&p).is_err());
}

#[test]
fn bedrock_legacy_iam_retains_region_inference_without_new_template_endpoint_limits() {
    for base in [
        "https://bedrock-runtime.cn-north-1.amazonaws.com.cn/prefix",
        "https://bedrock.us-east-1.amazonaws.com:8443",
    ] {
        let mut p = payload("bearer");
        p.auth_mode = None;
        p.api_key.clear();
        p.base_url = base.into();
        p.extra_body = serde_json::from_value(json!({"AWS_ACCESS_KEY_ID":" AKIAFIXTURE ", "aws-secret-access-key":" secret-fixture "})).unwrap();
        let headers =
            sign_bedrock_runtime_headers_if_needed(&p, &plan(&p, "fixture"), HeaderMap::new())
                .unwrap();
        assert!(headers["authorization"]
            .to_str()
            .unwrap()
            .starts_with("AWS4-HMAC-SHA256 "));
    }
}
