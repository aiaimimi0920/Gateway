use std::time::Duration;

use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::udio;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::browser_worker_runtime_helpers::udio_browser_worker_script_path;
use crate::upstream::browser_worker_types::UdioBrowserWorkerSuccess;
use crate::upstream::client::UpstreamClient;
use crate::upstream::udio_response_helpers::{
    build_udio_browser_executor_service_result, build_udio_non_image_generation_response,
    parse_udio_remote_browser_worker_verified_result, prepare_udio_execution_context,
};
use crate::upstream::udio_runtime_helpers::{
    build_udio_browser_executor_payload, build_udio_browser_worker_input,
    prepare_udio_browser_executor_service_input,
};

mod admission;
mod assets;
mod browser_worker;
#[cfg(test)]
mod tests;

impl UpstreamClient {
    pub(crate) async fn execute_udio_browser_executor_service_invocation(
        &self,
        input: &Value,
    ) -> Result<Value, GatewayError> {
        let prepared = prepare_udio_browser_executor_service_input(input)?;
        let result = self
            .execute_udio_browser_worker(
                &prepared.base_url,
                prepared.runtime_state_object_key.as_deref(),
                &prepared.headers,
                &prepared.request_body,
                prepared.target_asset_kind,
                prepared.wait_audio,
                prepared.wait_timeout,
                prepared.poll_interval,
                prepared.timeout,
            )
            .await?;
        admission::admit_songs(&result, prepared.target_asset_kind, prepared.wait_audio)?;
        Ok(build_udio_browser_executor_service_result(&result))
    }

    pub(crate) async fn execute_udio_media(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let prepared =
            prepare_udio_execution_context(payload, req, model, extra_headers, self.timeout)?;
        // Remote dispatch, local admission/worker and finalization use one async deadline.
        // Failure after submission is uncertain, never permission to resubmit or switch accounts.
        let deadline = tokio::time::Instant::now() + prepared.request_timeout;
        tokio::time::timeout_at(deadline, async {
            let worker_result = if let Some(result) = self
                .execute_remote_browser_executor(
                    "udio",
                    provider_account_id,
                    req.endpoint_kind,
                    build_udio_browser_executor_payload(
                        &prepared.base_url,
                        prepared.runtime_state_object_key.as_deref(),
                        &prepared.headers,
                        &prepared.generation_plan.generate_request,
                        prepared.generation_plan.output_kind,
                        prepared.generation_plan.wait_audio,
                        prepared.generation_plan.wait_timeout,
                        prepared.generation_plan.poll_interval,
                        prepared.request_timeout,
                        std::env::var("UDIO_BROWSER_EXECUTABLE_PATH").ok(),
                    ),
                )
                .await?
            {
                parse_udio_remote_browser_worker_verified_result(result)?
            } else {
                self.execute_udio_browser_worker(
                    &prepared.base_url,
                    prepared.runtime_state_object_key.as_deref(),
                    &prepared.headers,
                    &prepared.generation_plan.generate_request,
                    prepared.generation_plan.output_kind,
                    prepared.generation_plan.wait_audio,
                    prepared.generation_plan.wait_timeout,
                    prepared.generation_plan.poll_interval,
                    prepared.request_timeout,
                )
                .await?
            };

            let latest_songs = admission::admit_songs(
                &worker_result,
                prepared.generation_plan.output_kind,
                prepared.generation_plan.wait_audio,
            )?;

            match prepared.generation_plan.output_kind {
                udio::UdioOutputKind::Image => {
                    assets::finalize(
                        req,
                        &prepared.generation_plan.prompt,
                        &latest_songs,
                        deadline,
                    )
                    .await
                }
                udio::UdioOutputKind::Music | udio::UdioOutputKind::Video => {
                    build_udio_non_image_generation_response(
                        prepared.generation_plan.output_kind,
                        model,
                        &prepared.generation_plan.prompt,
                        &latest_songs,
                        worker_result.completed,
                        worker_result.message.as_deref(),
                    )
                }
            }
        })
        .await
        .map_err(|_| browser_worker::timeout_error())?
        .map_err(browser_worker::no_replay)
    }

    async fn execute_udio_browser_worker(
        &self,
        base_url: &str,
        runtime_state_object_key: Option<&str>,
        headers: &rquest::header::HeaderMap,
        request_body: &Value,
        target_asset_kind: udio::UdioOutputKind,
        wait_audio: bool,
        wait_timeout: Duration,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<UdioBrowserWorkerSuccess, GatewayError> {
        let script_path = udio_browser_worker_script_path();
        let input = build_udio_browser_worker_input(
            base_url,
            runtime_state_object_key,
            headers,
            request_body,
            target_asset_kind,
            wait_audio,
            wait_timeout,
            poll_interval,
            timeout,
            std::env::var("UDIO_BROWSER_EXECUTABLE_PATH").ok(),
        );
        let stdin_json = serde_json::to_vec(&input).map_err(|error| {
            udio::browser_worker_input_serialize_error(error.to_string().as_str())
        })?;

        browser_worker::run(
            crate::upstream::PreparedProducerBrowserWorkerLaunch {
                node_bin: std::env::var("UDIO_BROWSER_NODE_BIN").unwrap_or_else(|_| "node".into()),
                script_path,
                stdin_json,
            },
            timeout,
        )
        .await
    }
}
