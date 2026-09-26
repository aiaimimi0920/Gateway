use super::test_support::*;
use super::*;
use crate::upstream::client::UpstreamClient;

#[test]
fn missing_gemini_business_runtime_error_matches_contract() {
    let error = missing_gemini_business_runtime_error();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.code.as_deref(),
        Some("missing_gemini_business_runtime")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Business credentials require extra_body runtime material (configId + session)."
    );
}

#[test]
fn missing_gemini_business_runtime_field_error_matches_contract() {
    let error = missing_gemini_business_runtime_field_error("configId");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.code.as_deref(),
        Some("missing_gemini_business_runtime_field")
    );
    assert_eq!(
        error.message.as_str(),
        "Missing required runtime field `configId`."
    );
}

#[test]
fn gemini_business_no_images_error_matches_contract() {
    let error = gemini_business_no_images_error();
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_business_compatible")
    );
    assert_eq!(error.code.as_deref(), Some("gemini_business_no_images"));
    assert_eq!(
        error.message.as_str(),
        "Gemini Business image request completed without any generated files."
    );
}

#[test]
fn invalid_image_upload_error_matches_contract() {
    let error = invalid_image_upload_error();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("invalid_image_upload"));
    assert_eq!(
        error.message.as_str(),
        "Gemini Business image uploads must be JSON objects."
    );
}

#[test]
fn missing_required_field_error_matches_contract() {
    let error = missing_required_field_error("prompt");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("missing_required_field"));
    assert_eq!(error.message.as_str(), "Missing required field `prompt`.");
}

#[test]
fn gemini_business_rate_limited_error_matches_contract() {
    let error = gemini_business_rate_limited_error("too many requests");
    assert_eq!(error.http_status, Some(429));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_business_compatible")
    );
    assert_eq!(error.code.as_deref(), Some("gemini_business_rate_limited"));
    assert_eq!(error.message.as_str(), "too many requests");
}

#[test]
fn gemini_business_upstream_error_matches_contract() {
    let error = gemini_business_upstream_error("internal upstream failure");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_business_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_business_upstream_error")
    );
    assert_eq!(error.message.as_str(), "internal upstream failure");
}

#[test]
fn gemini_business_unsupported_request_plan_error_matches_contract() {
    let error = unsupported_request_plan_error();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_business_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_gemini_business_endpoint")
    );
}

#[test]
fn plan_gemini_business_chat_endpoint_rejected_locally() {
    let payload = make_payload(
        "gemini_business_compatible",
        "https://biz-discoveryengine.googleapis.com/v1alpha",
    );
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
    let err = UpstreamClient::build_request_plan(&payload, &req, "nano-banana-pro", false)
        .expect_err("gemini business chat requests should be rejected");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(
        err.code.as_deref(),
        Some("unsupported_gemini_business_endpoint")
    );
}

#[test]
fn missing_gemini_business_prompt_error_matches_contract() {
    let error = missing_gemini_business_prompt_error();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("missing_prompt"));
    assert_eq!(
        error.message.as_str(),
        "Gemini Business image requests require a prompt."
    );
}

#[test]
fn invalid_image_response_format_error_matches_contract() {
    let error = invalid_image_response_format_error();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("invalid_image_response_format"));
    assert_eq!(
        error.message.as_str(),
        "response_format must be a string when provided."
    );
}

#[test]
fn unsupported_image_response_format_error_matches_contract() {
    let error = unsupported_image_response_format_error();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_image_response_format")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Business image endpoints currently support response_format=b64_json or url."
    );
}

#[test]
fn missing_input_image_error_matches_contract() {
    let error = missing_input_image_error();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("missing_input_image"));
    assert_eq!(
        error.message.as_str(),
        "Image edit requests require at least one input image."
    );
}
