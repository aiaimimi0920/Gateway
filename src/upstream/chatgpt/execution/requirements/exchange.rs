//! Bounded same-origin exchanges; failure diagnostics never echo upstream JSON or HTML.
use super::super::super::{
    common::{classify_chatgpt_web_text_response, ResponseStage},
    web_reverse::ChatGptWebRequestContext,
    PROVIDER,
};
use super::super::transport::post_json;
use crate::protocol::upstream_body::collect_bounded_upstream_charset_text_with_provider;
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use rquest::Client;
use serde_json::Value;
use std::{collections::HashMap, time::Duration};

pub(super) struct Exchange<'a> {
    pub http: &'a Client,
    pub timeout: Duration,
    pub payload: &'a ProviderAccountPayload,
    pub request_context: &'a ChatGptWebRequestContext,
    pub extra_headers: Option<&'a HashMap<String, String>>,
}

impl Exchange<'_> {
    pub(super) async fn send(
        &self,
        path: &str,
        body: &Value,
        stage: ResponseStage,
    ) -> Result<Value, GatewayError> {
        let response = post_json(
            self.http,
            self.timeout,
            self.payload,
            self.request_context,
            self.extra_headers,
            path,
            body,
            None,
            false,
            None,
        )
        .await?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let text = collect_bounded_upstream_charset_text_with_provider(
            response,
            "ChatGPT Web reverse requirements body",
            PROVIDER,
        )
        .await?;
        let label = stage.label();
        classify_chatgpt_web_text_response(stage, status, content_type.as_deref(), &text).map_err(
            |mut error| {
                error.message = format!(
                    "ChatGPT Web reverse upstream failure: stage={label} upstream_status={status}."
                );
                error
            },
        )?;
        let value: Value = serde_json::from_str(&text).map_err(|_| {
            GatewayError::server_error(format!(
                "ChatGPT Web requirements JSON is invalid: stage={label}."
            ))
            .with_provider(PROVIDER)
            .with_code("chatgpt_web_requirements_decode_failed")
        })?;
        if !value.is_object() {
            return Err(GatewayError::server_error(format!(
                "ChatGPT Web requirements payload is not an object: stage={label}."
            ))
            .with_provider(PROVIDER)
            .with_code("chatgpt_web_requirements_invalid_shape"));
        }
        Ok(value)
    }
}

pub(super) fn optional_token(data: &Value, name: &str) -> Result<Option<String>, GatewayError> {
    let Some(value) = data.get(name) else {
        return Ok(None);
    };
    if value.is_null() || value.as_str().is_some_and(|value| value.trim().is_empty()) {
        return Ok(None);
    }
    let value = value
        .as_str()
        .map(str::trim)
        .filter(|value| value.len() <= 32768 && !value.chars().any(char::is_control))
        .ok_or_else(|| {
            GatewayError::server_error("ChatGPT Web requirements token has an invalid shape.")
                .with_provider(PROVIDER)
                .with_code("chatgpt_web_requirements_invalid_token")
        })?;
    Ok(Some(value.to_string()))
}

pub(super) fn required_token(data: &Value, name: &str) -> Result<String, GatewayError> {
    optional_token(data, name)?.ok_or_else(|| {
        GatewayError::server_error(
            "ChatGPT Web requirements response is missing the required token.",
        )
        .with_provider(PROVIDER)
        .with_code("chatgpt_web_requirements_missing_token")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn phase_tokens_are_bounded_and_optional_empty_so_is_compatible() {
        for value in [json!(null), json!(""), json!(" ")] {
            assert!(optional_token(&json!({"so_token":value}), "so_token")
                .unwrap()
                .is_none());
        }
        for value in [
            json!("x".repeat(32769)),
            json!("private\nmarker"),
            json!(false),
        ] {
            assert!(required_token(&json!({"token":value}), "token").is_err());
        }
    }
}
