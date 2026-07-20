use std::process::Stdio;
use std::time::Duration;

use rquest::Method;
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::udio;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::browser_worker_runtime_helpers::udio_browser_worker_script_path;
use crate::upstream::browser_worker_types::UdioBrowserWorkerSuccess;
use crate::upstream::client::UpstreamClient;
use crate::upstream::udio_response_helpers::{
    build_udio_browser_executor_service_result, build_udio_downloaded_images_response,
    build_udio_non_image_generation_response, ensure_successful_udio_media_fetch_status,
    materialize_udio_downloaded_image, parse_udio_browser_worker_verified_output,
    parse_udio_remote_browser_worker_verified_result, prepare_udio_execution_context,
    resolve_udio_image_generation_plan, resolve_udio_worker_latest_songs,
};
use crate::upstream::udio_runtime_helpers::{
    build_udio_browser_executor_payload, build_udio_browser_worker_input,
    prepare_udio_browser_executor_service_input,
};

impl UpstreamClient {
    pub(crate) async fn execute_udio_browser_executor_service_invocation(
        &self,
        input: &Value,
    ) -> Result<Value, GatewayError> {
        let prepared = prepare_udio_browser_executor_service_input(input)?;
        let result = self
            .execute_udio_browser_worker(
                "udio_compatible",
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
        let provider = "udio_compatible";
        let prepared =
            prepare_udio_execution_context(payload, req, model, extra_headers, self.timeout)?;
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
                provider,
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

        let latest_songs = resolve_udio_worker_latest_songs(&worker_result)?;

        match prepared.generation_plan.output_kind {
            udio::UdioOutputKind::Image => {
                let (image_urls, url_response) = resolve_udio_image_generation_plan(
                    req,
                    &prepared.generation_plan.prompt,
                    &latest_songs,
                )?;
                if let Some(response) = url_response {
                    return Ok(response);
                }

                let mut images = Vec::with_capacity(image_urls.len());
                for url in &image_urls {
                    let response = self
                        .http
                        .request(Method::GET, url)
                        .timeout(prepared.request_timeout)
                        .send()
                        .await
                        .map_err(|e| classify_network_error(&e, Some(provider)))?;
                    let status = response.status().as_u16();
                    let response_headers = response.headers().clone();
                    if !response.status().is_success() {
                        let body_text = response
                            .text()
                            .await
                            .unwrap_or_else(|_| String::from("<unreadable body>"));
                        ensure_successful_udio_media_fetch_status(
                            status,
                            &response_headers,
                            &body_text,
                        )?;
                        unreachable!("non-2xx Udio image downloads should fail");
                    }

                    let bytes = response
                        .bytes()
                        .await
                        .map_err(|e| classify_network_error(&e, Some(provider)))?;
                    images.push(materialize_udio_downloaded_image(
                        &response_headers,
                        bytes.as_ref(),
                    ));
                }

                Ok(build_udio_downloaded_images_response(
                    req,
                    &prepared.generation_plan.prompt,
                    &images,
                ))
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
    }

    async fn execute_udio_browser_worker(
        &self,
        provider: &str,
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

        let mut child = Command::new(
            std::env::var("UDIO_BROWSER_NODE_BIN").unwrap_or_else(|_| "node".to_string()),
        )
        .arg(&script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            udio::browser_worker_spawn_failed_error(
                script_path.as_path(),
                error.to_string().as_str(),
            )
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&stdin_json)
                .await
                .map_err(|error| udio::browser_worker_stdin_error(error.to_string().as_str()))?;
        }

        let output = tokio::time::timeout(timeout, child.wait_with_output())
            .await
            .map_err(|_| udio::browser_worker_timeout_error())?
            .map_err(|error| udio::browser_worker_wait_failed_error(error.to_string().as_str()))?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        parse_udio_browser_worker_verified_output(&stdout, &stderr, provider)
    }
}
