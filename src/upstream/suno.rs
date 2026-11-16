use std::time::Duration;

use rquest::Method;
use serde_json::Value;

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::suno;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::browser_worker_runtime_helpers::suno_browser_worker_script_path;
use crate::upstream::browser_worker_types::SunoBrowserWorkerSuccess;
use crate::upstream::client::UpstreamClient;
use crate::upstream::suno_response_helpers::{
    build_suno_browser_executor_service_result, build_suno_downloaded_images_response,
    build_suno_non_image_generation_response, ensure_successful_suno_media_fetch_status,
    materialize_suno_downloaded_image, parse_suno_remote_browser_worker_verified_result,
    resolve_suno_image_generation_plan,
};
use crate::upstream::suno_runtime_helpers::{
    build_suno_browser_executor_payload, build_suno_browser_worker_input,
    prepare_suno_browser_executor_service_input, prepare_suno_execution_context,
    unsupported_request_plan_error,
};

mod assets;
mod browser_binding;
mod browser_worker;
mod direct;
mod direct_state;
#[cfg(test)]
mod tests;
mod transport;

pub(crate) fn request_plan_unsupported_error() -> GatewayError {
    unsupported_request_plan_error()
}

impl UpstreamClient {
    pub(crate) async fn execute_suno_browser_executor_service_invocation(
        &self,
        input: &Value,
    ) -> Result<Value, GatewayError> {
        let prepared = prepare_suno_browser_executor_service_input(input)?;
        let expected_model = browser_binding::worker_model(&prepared.request_body)?;
        let result = self
            .execute_suno_browser_worker(
                "suno_compatible",
                &prepared.base_url,
                &prepared.headers,
                &prepared.request_body,
                &prepared.target_asset_kind,
                prepared.wait_completion,
                prepared.wait_timeout,
                prepared.poll_interval,
                prepared.timeout,
            )
            .await?;
        browser_binding::verify(&result, expected_model)?;
        Ok(build_suno_browser_executor_service_result(&result))
    }

    pub(crate) async fn execute_suno_media(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let prepared =
            prepare_suno_execution_context(payload, req, extra_headers, false, self.timeout)?;
        // One owner bounds challenge, submission, polling and finalization. Cancellation
        // drops the in-flight request; it cannot cancel a generation already accepted remotely.
        let deadline = tokio::time::Instant::now()
            + prepared
                .execution_plan
                .request_timeout
                .min(prepared.execution_plan.wait_timeout);
        tokio::time::timeout_at(deadline, async {
            let (clips, completed) = direct::run(self, &prepared, req, model, deadline).await?;
            self.finalize_suno_media_response(
                req,
                model,
                &prepared.prompt,
                &clips,
                completed,
                None,
                deadline.saturating_duration_since(tokio::time::Instant::now()),
            )
            .await
        })
        .await
        .map_err(|_| transport::timeout())?
        .map_err(transport::no_replay)
    }

    pub(crate) async fn execute_suno_browser_backed(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let provider = "suno_compatible";
        let browser_request = browser_binding::prepare(req, model)?;
        let expected_model = browser_binding::worker_model(&browser_request.raw_body)?;
        let prepared =
            prepare_suno_execution_context(payload, req, extra_headers, true, self.timeout)?;

        let deadline = tokio::time::Instant::now() + prepared.execution_plan.request_timeout;
        // Admission/queue, local or remote worker and finalization share the same budget.
        // A worker may already have submitted a paid generation: never replay on error.
        tokio::time::timeout_at(deadline, async {
            let worker_result = if let Some(result) = self
                .execute_remote_browser_executor(
                    "suno",
                    provider_account_id,
                    req.endpoint_kind,
                    build_suno_browser_executor_payload(
                        &prepared.base_url,
                        &prepared.runtime_headers,
                        &browser_request,
                        prepared.execution_plan.target_asset_kind,
                        prepared.execution_plan.wait_timeout,
                        prepared.execution_plan.poll_interval,
                        prepared.execution_plan.request_timeout,
                        std::env::var("SUNO_BROWSER_EXECUTABLE_PATH").ok(),
                    ),
                )
                .await?
            {
                parse_suno_remote_browser_worker_verified_result(result)?
            } else {
                self.execute_suno_browser_worker(
                    provider,
                    payload.base_url.trim_end_matches('/'),
                    &prepared.runtime_headers,
                    &browser_request.raw_body,
                    prepared.execution_plan.target_asset_kind,
                    prepared.execution_plan.wait_completion,
                    prepared.execution_plan.wait_timeout,
                    prepared.execution_plan.poll_interval,
                    prepared.execution_plan.request_timeout,
                )
                .await?
            };
            browser_binding::verify(&worker_result, expected_model)?;

            self.finalize_suno_media_response(
                req,
                model,
                &prepared.prompt,
                &worker_result.clips,
                worker_result.completed,
                worker_result.message.as_deref(),
                deadline.saturating_duration_since(tokio::time::Instant::now()),
            )
            .await
        })
        .await
        .map_err(|_| browser_worker::timeout_error())?
        .map_err(transport::no_replay)
    }

    async fn finalize_suno_media_response(
        &self,
        req: &CanonicalRelayRequest,
        model: &str,
        prompt: &str,
        clips: &[suno::SunoClip],
        completed: bool,
        message: Option<&str>,
        request_timeout: Duration,
    ) -> Result<Value, GatewayError> {
        let provider = "suno_compatible";
        match req.endpoint_kind {
            EndpointKind::ImagesGenerations => {
                let (image_urls, url_response) =
                    resolve_suno_image_generation_plan(req, prompt, clips)?;
                if let Some(response) = url_response {
                    return Ok(response);
                }
                let mut images = Vec::with_capacity(image_urls.len());
                for url in image_urls {
                    let response = self
                        .http
                        .request(Method::GET, &url)
                        .timeout(request_timeout)
                        .send()
                        .await
                        .map_err(|error| classify_network_error(&error, Some(provider)))?;
                    let status = response.status().as_u16();
                    let response_headers = response.headers().clone();
                    if !(200..300).contains(&status) {
                        let body_text = response.text().await.unwrap_or_default();
                        ensure_successful_suno_media_fetch_status(
                            status,
                            &response_headers,
                            &body_text,
                        )?;
                        unreachable!("non-2xx Suno image downloads should fail");
                    }
                    let bytes = response
                        .bytes()
                        .await
                        .map_err(|error| classify_network_error(&error, Some(provider)))?;
                    images.push(materialize_suno_downloaded_image(
                        &response_headers,
                        &url,
                        bytes.as_ref(),
                    ));
                }
                build_suno_downloaded_images_response(req, prompt, &images)
            }
            EndpointKind::MusicGenerations => {
                if completed {
                    assets::verify(clips, tokio::time::Instant::now() + request_timeout).await?;
                }
                build_suno_non_image_generation_response(
                    req.endpoint_kind,
                    model,
                    prompt,
                    clips,
                    completed,
                    message,
                )
            }
            EndpointKind::VideosGenerations => build_suno_non_image_generation_response(
                req.endpoint_kind,
                model,
                prompt,
                clips,
                completed,
                message,
            ),
            _ => Err(suno::unsupported_media_endpoint_error()),
        }
    }

    async fn execute_suno_browser_worker(
        &self,
        _provider: &str,
        base_url: &str,
        headers: &rquest::header::HeaderMap,
        request_body: &Value,
        target_asset_kind: &str,
        wait_completion: bool,
        wait_timeout: Duration,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<SunoBrowserWorkerSuccess, GatewayError> {
        let script_path = suno_browser_worker_script_path();
        let input = build_suno_browser_worker_input(
            base_url,
            headers,
            request_body,
            target_asset_kind,
            wait_completion,
            wait_timeout,
            poll_interval,
            timeout,
            std::env::var("SUNO_BROWSER_EXECUTABLE_PATH").ok(),
        );
        let stdin_json = serde_json::to_vec(&input).map_err(|error| {
            suno::browser_worker_input_serialize_error(error.to_string().as_str())
        })?;

        browser_worker::run(
            crate::upstream::PreparedProducerBrowserWorkerLaunch {
                node_bin: std::env::var("SUNO_BROWSER_NODE_BIN")
                    .unwrap_or_else(|_| "node".to_string()),
                script_path,
                stdin_json,
            },
            timeout,
        )
        .await
    }
}
