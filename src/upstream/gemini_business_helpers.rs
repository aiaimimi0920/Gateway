use crate::error::GatewayError;
use serde_json::Value;

pub(crate) fn gemini_business_unsupported_images_endpoint_error() -> GatewayError {
    GatewayError::bad_request(
        "Gemini Business adapters only support /v1/images/generations and /v1/images/edits.",
    )
    .with_provider("gemini_business_compatible")
    .with_code("unsupported_gemini_business_endpoint")
}

pub(crate) fn gemini_business_missing_file_id_error(provider: &str) -> GatewayError {
    GatewayError::server_error(
        "Gemini Business upload response missing addContextFileResponse.fileId",
    )
    .with_provider(provider)
    .with_code("gemini_business_missing_file_id")
}

pub(crate) fn gemini_business_invalid_stream_json_root_error(
    provider: &str,
    root: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Business returned unsupported widget stream JSON root: {root}"
    ))
    .with_provider(provider)
    .with_code("gemini_business_invalid_stream_json")
}

pub(crate) fn gemini_business_invalid_stream_json_error(
    provider: &str,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Business returned invalid widget stream JSON: {error}"
    ))
    .with_provider(provider)
    .with_code("gemini_business_invalid_stream_json")
}

pub(crate) fn extract_gemini_business_upload_file_id(
    upload_json: &Value,
    provider: &str,
) -> Result<String, GatewayError> {
    upload_json
        .get("addContextFileResponse")
        .and_then(|v| {
            v.get("fileId")
                .or_else(|| v.get("contextFile").and_then(|file| file.get("fileId")))
        })
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| gemini_business_missing_file_id_error(provider))
}

pub(crate) fn parse_gemini_business_stream_response_objects(
    body_text: &str,
    provider: &str,
) -> Result<Vec<Value>, GatewayError> {
    match serde_json::from_str::<Value>(body_text) {
        Ok(Value::Array(items)) => Ok(items),
        Ok(Value::Object(object)) => Ok(vec![Value::Object(object)]),
        Ok(other) => {
            let root = other.to_string();
            Err(gemini_business_invalid_stream_json_root_error(
                provider,
                root.as_str(),
            ))
        }
        Err(error) => {
            let error_text = error.to_string();
            Err(gemini_business_invalid_stream_json_error(
                provider,
                error_text.as_str(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        extract_gemini_business_upload_file_id, gemini_business_invalid_stream_json_error,
        gemini_business_invalid_stream_json_root_error, gemini_business_missing_file_id_error,
        gemini_business_unsupported_images_endpoint_error,
        parse_gemini_business_stream_response_objects,
    };
    use serde_json::json;

    #[test]
    fn gemini_business_unsupported_images_endpoint_error_matches_contract() {
        let error = gemini_business_unsupported_images_endpoint_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_business_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_gemini_business_endpoint")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Business adapters only support /v1/images/generations and /v1/images/edits."
        );
    }

    #[test]
    fn gemini_business_missing_file_id_error_matches_contract() {
        let error = gemini_business_missing_file_id_error("gemini_business_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_business_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_business_missing_file_id")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Business upload response missing addContextFileResponse.fileId"
        );
    }

    #[test]
    fn extract_gemini_business_upload_file_id_accepts_nested_context_file_shape() {
        let file_id = extract_gemini_business_upload_file_id(
            &json!({
                "addContextFileResponse": {
                    "contextFile": {
                        "fileId": "file-123"
                    }
                }
            }),
            "gemini_business_compatible",
        )
        .expect("nested upload file id");
        assert_eq!(file_id, "file-123");
    }

    #[test]
    fn parse_gemini_business_stream_response_objects_accepts_single_object_root() {
        let objects = parse_gemini_business_stream_response_objects(
            "{\"streamAssistResponse\":{\"answer\":{}}}",
            "gemini_business_compatible",
        )
        .expect("single object stream root");
        assert_eq!(objects.len(), 1);
        assert!(objects[0].get("streamAssistResponse").is_some());
    }

    #[test]
    fn parse_gemini_business_stream_response_objects_rejects_scalar_root() {
        let error = parse_gemini_business_stream_response_objects(
            "\"unsupported\"",
            "gemini_business_compatible",
        )
        .expect_err("scalar root should fail");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_business_invalid_stream_json")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Business returned unsupported widget stream JSON root: \"unsupported\""
        );
    }

    #[test]
    fn gemini_business_invalid_stream_json_root_error_matches_contract() {
        let error = gemini_business_invalid_stream_json_root_error(
            "gemini_business_compatible",
            "{\"unsupported\":true}",
        );
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_business_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_business_invalid_stream_json")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Business returned unsupported widget stream JSON root: {\"unsupported\":true}"
        );
    }

    #[test]
    fn gemini_business_invalid_stream_json_error_matches_contract() {
        let error = gemini_business_invalid_stream_json_error(
            "gemini_business_compatible",
            "expected value at line 1 column 1",
        );
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_business_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_business_invalid_stream_json")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Business returned invalid widget stream JSON: expected value at line 1 column 1"
        );
    }
}
