use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::lumalabs;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;
use crate::upstream::lumalabs_response_helpers::{
    build_lumalabs_non_image_generation_response, prepare_lumalabs_execution_context,
};

mod assets;
mod browser_worker;
mod session;
mod task_result;
#[cfg(test)]
mod task_tests;
#[cfg(test)]
mod tests;
use crate::upstream::lumalabs_runtime_helpers::{
    build_lumalabs_browser_executor_payload_from_prepared,
    prepare_lumalabs_browser_executor_service_input, prepare_lumalabs_browser_worker_launch,
    PreparedLumalabsBrowserExecutorServiceInput,
};

/// Preflight the existing plan without dispatching or inferring a model/action mapping.
pub(crate) fn validate_media_probe(
    payload: &ProviderAccountPayload,
    request: &CanonicalRelayRequest,
    model: &str,
) -> Result<(), GatewayError> {
    crate::upstream::lumalabs_response_helpers::prepare_lumalabs_media_plan(
        payload,
        request,
        model,
        std::time::Duration::ZERO,
    )
    .map(|_| ())
}

fn legacy_executor_result(
    value: Value,
    prepared: &PreparedLumalabsBrowserExecutorServiceInput,
) -> Result<Value, GatewayError> {
    let task = browser_worker::parse_task(value, "lumalabs_compatible")?;
    task.validate(prepared)?;
    Ok(task.into_value())
}

impl UpstreamClient {
    pub(crate) async fn execute_lumalabs_browser_executor_service_invocation(
        &self,
        input: &Value,
        retain_session: bool,
    ) -> Result<Value, GatewayError> {
        if retain_session
            && !input
                .get("sessionBinding")
                .and_then(Value::as_str)
                .is_some_and(|binding| uuid::Uuid::parse_str(binding).is_ok())
        {
            return Err(
                GatewayError::bad_request("Luma Web executor requires a session binding.")
                    .with_code("lumalabs_session_binding_invalid"),
            );
        }
        let prepared = prepare_lumalabs_browser_executor_service_input(input)?;
        if retain_session {
            // Only the versioned route can return a receipt, including on a business failure.
            return self.execute_luma_worker_value(&prepared, input).await;
        }
        // The legacy endpoint keeps its old result shape and business error classification.
        let value = self
            .execute_luma_worker_value(&prepared, &Value::Null)
            .await?;
        legacy_executor_result(value, &prepared)
    }

    pub(crate) async fn execute_lumalabs_media(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let provider = "lumalabs_compatible";
        let prepared = prepare_lumalabs_execution_context(payload, req, model, self.timeout)?;
        // One async deadline covers remote/local dispatch and response finalization. Any failure
        // after entering this owner is uncertain, never permission to generate on another account.
        let deadline = tokio::time::Instant::now() + prepared.browser_input.timeout;
        tokio::time::timeout_at(deadline, async {
            let result = self
                .dispatch_luma_session(
                    provider_account_id,
                    payload,
                    req.endpoint_kind,
                    prepared.browser_input.clone(),
                    false,
                )
                .await?
                .expect("business dispatch is never skipped");
            let task = browser_worker::parse_task(result, provider)?;
            task.validate(&prepared.browser_input)?;
            let signed_url = task.signed_url;

            match prepared.media_plan.operation {
                lumalabs::LumalabsMediaOperation::Image => {
                    // A URL is not a successful image. Both envelopes require the same validated
                    // asset; never use the account/client/caller headers to fetch an untrusted URL.
                    let (bytes, mime) = assets::download(&signed_url, deadline).await?;
                    Ok(if lumalabs::prefers_url_response(req)? {
                        lumalabs::build_openai_images_response_from_url(
                            req,
                            &prepared.media_plan.prompt,
                            &signed_url,
                        )
                    } else {
                        lumalabs::build_openai_images_response_from_bytes(
                            &prepared.media_plan.prompt,
                            mime,
                            &bytes,
                        )
                    })
                }
                lumalabs::LumalabsMediaOperation::Video
                | lumalabs::LumalabsMediaOperation::Audio => {
                    Ok(build_lumalabs_non_image_generation_response(
                        prepared.media_plan.operation,
                        model,
                        &prepared.media_plan.prompt,
                        &signed_url,
                    ))
                }
            }
        })
        .await
        .map_err(|_| browser_worker::timeout_error())?
        .map_err(browser_worker::no_replay)
    }
}
