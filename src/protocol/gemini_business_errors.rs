use crate::error::GatewayError;

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Gemini Business adapters currently support only image generation/edit passthrough endpoints",
    )
    .with_provider("gemini_business_compatible")
    .with_code("unsupported_gemini_business_endpoint")
}

pub fn missing_input_image_error() -> GatewayError {
    GatewayError::bad_request("Image edit requests require at least one input image.")
        .with_code("missing_input_image")
}

pub fn missing_gemini_business_runtime_error() -> GatewayError {
    GatewayError::bad_request(
        "Gemini Business credentials require extra_body runtime material (configId + session).",
    )
    .with_code("missing_gemini_business_runtime")
}

pub fn missing_gemini_business_runtime_field_error(field: &str) -> GatewayError {
    GatewayError::bad_request(format!("Missing required runtime field `{field}`."))
        .with_code("missing_gemini_business_runtime_field")
}

pub fn missing_gemini_business_prompt_error() -> GatewayError {
    GatewayError::bad_request("Gemini Business image requests require a prompt.")
        .with_code("missing_prompt")
}

pub fn invalid_image_response_format_error() -> GatewayError {
    GatewayError::bad_request("response_format must be a string when provided.")
        .with_code("invalid_image_response_format")
}

pub fn unsupported_image_response_format_error() -> GatewayError {
    GatewayError::bad_request(
        "Gemini Business image endpoints currently support response_format=b64_json or url.",
    )
    .with_code("unsupported_image_response_format")
}

pub fn gemini_business_no_images_error() -> GatewayError {
    GatewayError::server_error(
        "Gemini Business image request completed without any generated files.",
    )
    .with_provider("gemini_business_compatible")
    .with_code("gemini_business_no_images")
}

pub fn gemini_business_rate_limited_error(message: &str) -> GatewayError {
    GatewayError::rate_limited(message, 1_000)
        .with_provider("gemini_business_compatible")
        .with_code("gemini_business_rate_limited")
}

pub fn gemini_business_upstream_error(message: &str) -> GatewayError {
    GatewayError::server_error(message)
        .with_provider("gemini_business_compatible")
        .with_code("gemini_business_upstream_error")
}

pub fn invalid_image_upload_error() -> GatewayError {
    GatewayError::bad_request("Gemini Business image uploads must be JSON objects.")
        .with_code("invalid_image_upload")
}

pub fn missing_required_field_error(field: &str) -> GatewayError {
    GatewayError::bad_request(format!("Missing required field `{field}`."))
        .with_code("missing_required_field")
}
