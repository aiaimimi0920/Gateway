//! Shared image executor; typed current Web never guesses legacy account-level settings.
use crate::{
    error::{classify_upstream_error, sanitize_provider_error_message, FallbackHint, GatewayError},
    protocol::{canonical::CanonicalRelayRequest, chataibot},
    routing::candidate::ProviderAccountPayload,
    upstream::client::UpstreamClient,
};
use serde_json::Value;
mod admission;
mod assets;
#[cfg(test)]
mod boundary_tests;
#[cfg(test)]
mod contract_tests;
mod jobs;
#[cfg(test)]
mod pipeline_failures;
#[cfg(test)]
mod pipeline_fixture;
#[cfg(test)]
mod pipeline_tests;
#[cfg(test)]
mod probe_failures;
#[cfg(test)]
mod probe_management_tests;
#[cfg(test)]
mod probe_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
mod transport;

/// Manual tests share product admission with ordinary requests, without sending or mutating settings.
pub(crate) fn validate_media_probe(
    payload: &ProviderAccountPayload,
    request: &CanonicalRelayRequest,
    model: &str,
) -> Result<(), GatewayError> {
    if !admission::typed(payload) {
        return Err(unsupported_request_plan_error());
    }
    admission::Prepared::new(payload, request, model).map(|_| ())
}

fn no_replay(mut error: GatewayError) -> GatewayError {
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "ChatAIBot generation must not be replayed.".into(),
    };
    error
}
fn failure(code: &str, message: &str) -> GatewayError {
    let mut error = GatewayError::server_error(message)
        .with_provider("chataibot_compatible")
        .with_code(code);
    error.http_status = Some(502);
    no_replay(error)
}
fn parse_chataibot_http_response(
    status: u16,
    content_type: Option<&str>,
    body_text: &str,
    provider: &str,
) -> Result<Value, GatewayError> {
    if !(200..300).contains(&status) {
        return Err(no_replay(classify_upstream_error(
            status,
            body_text,
            Some(provider),
        )));
    }
    if body_text.trim().is_empty() {
        return Err(failure(
            "chataibot_empty_response",
            "ChatAIBot returned an empty successful response.",
        ));
    }
    serde_json::from_str(body_text).map_err(|_| {
        failure(
            "chataibot_invalid_response",
            &format!(
                "ChatAIBot returned non-JSON content ({}); response body omitted.",
                sanitize_provider_error_message(content_type.unwrap_or("<missing>"))
            ),
        )
    })
}
pub(crate) fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Chataibot adapters currently support only image generation/edit passthrough endpoints",
    )
    .with_code("unsupported_chataibot_endpoint")
}
impl UpstreamClient {
    pub(crate) async fn execute_chataibot_images(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let prepared = admission::Prepared::new(payload, req, model)?;
        let headers = admission::headers(payload, extra_headers)?;
        let duration = self.timeout.min(std::time::Duration::from_secs(1200));
        if duration.is_zero() {
            return Err(transport::timeout());
        }
        // Caller task owns submit, same-job polling, DNS, asset and decode. No detached generation.
        tokio::time::timeout(duration, async {
            let deadline = tokio::time::Instant::now() + duration;
            let mut total = 0;
            if !admission::typed(payload) {
                // Compatibility only: the current public SPA does not prove this legacy ratio field.
                transport::send(
                    &self.http,
                    payload,
                    &headers,
                    "/api/user/update",
                    Some(admission::Body::Json(
                        chataibot::build_update_settings_request(
                            &chataibot::aspect_ratio_from_request(req),
                        ),
                    )),
                    deadline,
                    &mut total,
                    false,
                )
                .await?;
            }
            let body = transport::send(
                &self.http,
                payload,
                &headers,
                prepared.path,
                Some(prepared.body),
                deadline,
                &mut total,
                true,
            )
            .await?;
            let body =
                jobs::finish(&self.http, payload, &headers, body, deadline, &mut total).await?;
            let urls = chataibot::extract_image_urls(&body).map_err(|_| {
                failure(
                    "chataibot_missing_image_url",
                    "ChatAIBot did not return a usable image; response omitted.",
                )
            })?;
            let url = urls.first().ok_or_else(|| {
                failure("chataibot_missing_image_url", "ChatAIBot image is missing.")
            })?;
            let image = assets::download(url, deadline).await?;
            if prepared.url_response {
                chataibot::build_openai_images_response_from_urls(
                    req,
                    &prepared.prompt,
                    std::slice::from_ref(url),
                )
            } else {
                Ok(chataibot::build_openai_images_response_from_bytes(
                    req,
                    &prepared.prompt,
                    &[image],
                ))
            }
        })
        .await
        .map_err(|_| transport::timeout())?
    }
}
