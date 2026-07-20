use aws_credential_types::Credentials;
use aws_sigv4::http_request::{
    sign as aws_sign_http_request, SignableBody, SignableRequest, SigningSettings,
};
use aws_sigv4::sign::v4;
use aws_smithy_runtime_api::client::identity::Identity;
use rquest::header::HeaderMap;
use std::time::SystemTime;

use crate::error::GatewayError;
use crate::protocol::kiro;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::{insert_header_map_value, RequestPlan};
use crate::upstream::openai_compatible_common::decode_raw_request_body;

#[derive(Clone, Debug)]
struct BedrockRuntimeSigningCredentials {
    access_key_id: String,
    secret_access_key: String,
    session_token: Option<String>,
    region: String,
}

pub(crate) fn sign_bedrock_runtime_headers_if_needed(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
    mut headers: HeaderMap,
) -> Result<HeaderMap, GatewayError> {
    let Some(credentials) = bedrock_runtime_signing_credentials_from_payload(payload) else {
        return Ok(headers);
    };

    headers.remove("authorization");

    let mut request_url = url::Url::parse(&plan.url).map_err(|error| {
        GatewayError::server_error(format!("invalid bedrock request url: {error}"))
            .with_code("bedrock_sigv4_invalid_url")
    })?;
    if !plan.query.is_empty() {
        let mut query = request_url.query_pairs_mut();
        for (key, value) in &plan.query {
            query.append_pair(key, value);
        }
    }

    let mut signable_headers: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .filter(|(name, _)| !name.eq_ignore_ascii_case("authorization"))
        .collect();

    let host = request_url
        .host_str()
        .ok_or_else(|| {
            GatewayError::server_error("bedrock request url missing host")
                .with_code("bedrock_sigv4_missing_host")
        })?
        .to_string();
    if !signable_headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("host"))
    {
        signable_headers.push(("host".to_string(), host));
    }

    let body_bytes = if let Some(body) = &plan.body {
        if let Some((_content_type, bytes)) = decode_raw_request_body(body) {
            bytes.to_vec()
        } else {
            serde_json::to_vec(body).map_err(|error| {
                GatewayError::server_error(format!("serialize bedrock request body: {error}"))
                    .with_code("bedrock_sigv4_body_serialize_failed")
            })?
        }
    } else {
        Vec::new()
    };

    let identity: Identity = Credentials::new(
        credentials.access_key_id,
        credentials.secret_access_key,
        credentials.session_token,
        None,
        "bedrock-runtime-credentials",
    )
    .into();
    let signing_params = v4::SigningParams::builder()
        .identity(&identity)
        .region(&credentials.region)
        .name("bedrock")
        .time(SystemTime::now())
        .settings(SigningSettings::default())
        .build()
        .map_err(|error| {
            GatewayError::server_error(format!("build bedrock sigv4 params: {error}"))
                .with_code("bedrock_sigv4_params_failed")
        })?;
    let signing_params = aws_sigv4::http_request::SigningParams::from(signing_params);
    let signable_request = SignableRequest::new(
        plan.method.as_str(),
        request_url.as_str(),
        signable_headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str())),
        SignableBody::Bytes(body_bytes.as_slice()),
    )
    .map_err(|error| {
        GatewayError::server_error(format!("build signable bedrock request: {error}"))
            .with_code("bedrock_sigv4_signable_request_failed")
    })?;
    let (signing_instructions, _signature) =
        aws_sign_http_request(signable_request, &signing_params)
            .map_err(|error| {
                GatewayError::server_error(format!("sign bedrock request: {error}"))
                    .with_code("bedrock_sigv4_sign_failed")
            })?
            .into_parts();

    for (name, value) in signing_instructions.headers() {
        insert_header_map_value(&mut headers, name, value);
    }

    Ok(headers)
}

fn bedrock_runtime_signing_credentials_from_payload(
    payload: &ProviderAccountPayload,
) -> Option<BedrockRuntimeSigningCredentials> {
    let access_key_id = kiro::read_payload_string(
        payload.extra_body.as_ref(),
        &["awsAccessKeyId", "aws_access_key_id"],
    )?;
    let secret_access_key = kiro::read_payload_string(
        payload.extra_body.as_ref(),
        &["awsSecretAccessKey", "aws_secret_access_key"],
    )?;
    let session_token = kiro::read_payload_string(
        payload.extra_body.as_ref(),
        &["awsSessionToken", "aws_session_token"],
    );
    let region = kiro::read_payload_string(
        payload.extra_body.as_ref(),
        &["awsRegion", "aws_region", "region"],
    )
    .or_else(|| infer_bedrock_region_from_base_url(&payload.base_url))?;

    Some(BedrockRuntimeSigningCredentials {
        access_key_id,
        secret_access_key,
        session_token,
        region,
    })
}

fn infer_bedrock_region_from_base_url(base_url: &str) -> Option<String> {
    let host = url::Url::parse(base_url).ok()?.host_str()?.to_string();
    let prefixes = [
        "bedrock-runtime.",
        "bedrock-runtime-fips.",
        "bedrock.",
        "bedrock-fips.",
    ];
    for prefix in prefixes {
        if let Some(rest) = host.strip_prefix(prefix) {
            let region = rest
                .split('.')
                .next()
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            return Some(region.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::EndpointKind;
    use serde_json::json;
    use std::collections::HashMap;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_plan(url: &str) -> RequestPlan {
        RequestPlan {
            method: rquest::Method::POST,
            url: url.to_string(),
            query: vec![],
            body: Some(json!({
                "messages": [{"role":"user","content":"hello"}]
            })),
            response_kind: EndpointKind::ChatCompletions,
        }
    }

    #[test]
    fn bedrock_sigv4_headers_include_authorization_and_session_token() {
        let mut payload = make_payload(
            "bedrock_converse_compatible",
            "https://bedrock-runtime.us-east-1.amazonaws.com",
        );
        payload.api_key.clear();
        payload.extra_body = Some(
            serde_json::from_value(serde_json::json!({
                "awsAccessKeyId": "AKIAIOSFODNN7EXAMPLE",
                "awsSecretAccessKey": "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
                "awsSessionToken": "IQoJb3JpZ2luX2VjEOr//////////wEaCXVzLXdlc3QtMiJGMEQCIBEXAMPLESESSIONTOKEN",
                "awsRegion": "us-east-1"
            }))
            .expect("aws creds payload"),
        );
        let plan = make_plan("https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3-5-sonnet/converse");
        let headers = rquest::header::HeaderMap::new();

        let signed_headers =
            sign_bedrock_runtime_headers_if_needed(&payload, &plan, headers).expect("signed");

        let authorization = signed_headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        assert!(
            authorization.starts_with("AWS4-HMAC-SHA256 "),
            "expected AWS4 authorization header, got {authorization:?}"
        );
        assert!(signed_headers.get("x-amz-date").is_some());
        assert!(signed_headers.get("x-amz-security-token").is_some());
    }

    #[test]
    fn bedrock_sigv4_headers_infer_region_from_base_url_when_runtime_region_missing() {
        let mut payload = make_payload(
            "bedrock_converse_compatible",
            "https://bedrock-runtime.us-west-2.amazonaws.com",
        );
        payload.api_key.clear();
        payload.extra_body = Some(
            serde_json::from_value(json!({
                "awsAccessKeyId": "AKIAIOSFODNN7EXAMPLE",
                "awsSecretAccessKey": "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY"
            }))
            .expect("aws creds payload"),
        );
        let plan = make_plan(
            "https://bedrock-runtime.us-west-2.amazonaws.com/model/anthropic.claude-3-5-sonnet/converse",
        );
        let headers = rquest::header::HeaderMap::new();

        let signed_headers =
            sign_bedrock_runtime_headers_if_needed(&payload, &plan, headers).expect("signed");

        let authorization = signed_headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        assert!(
            authorization.contains("Credential=AKIAIOSFODNN7EXAMPLE/"),
            "expected credential scope in authorization header, got {authorization:?}"
        );
        assert!(signed_headers.get("x-amz-date").is_some());
    }
}
