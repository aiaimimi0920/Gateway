use std::process::Stdio;
use std::time::Duration;

use rquest::Method;
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::sleep;

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
    materialize_suno_downloaded_image, parse_suno_browser_worker_verified_output,
    parse_suno_challenge_probe_verified_response, parse_suno_feed_http_clips_response,
    parse_suno_generation_http_poll_seed, parse_suno_remote_browser_worker_verified_result,
    resolve_suno_image_generation_plan,
};
use crate::upstream::suno_runtime_helpers::{
    build_suno_browser_executor_payload, build_suno_browser_worker_input,
    build_suno_challenge_check_plan, build_suno_feed_poll_plan, build_suno_generate_plan,
    prepare_suno_browser_executor_service_input, prepare_suno_execution_context,
    unsupported_request_plan_error,
};

pub(crate) fn request_plan_unsupported_error() -> GatewayError {
    unsupported_request_plan_error()
}

impl UpstreamClient {
    pub(crate) async fn execute_suno_browser_executor_service_invocation(
        &self,
        input: &Value,
    ) -> Result<Value, GatewayError> {
        let prepared = prepare_suno_browser_executor_service_input(input)?;
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
        Ok(build_suno_browser_executor_service_result(&result))
    }

    pub(crate) async fn execute_suno_media(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let provider = "suno_compatible";
        let prepared =
            prepare_suno_execution_context(payload, req, extra_headers, false, self.timeout)?;

        let challenge_plan = build_suno_challenge_check_plan(&prepared.base_url, req.endpoint_kind);
        let challenge_response = self
            .send_plan(&challenge_plan, prepared.runtime_headers.clone())
            .timeout(prepared.execution_plan.request_timeout)
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let challenge_status = challenge_response.status().as_u16();
        let challenge_headers = challenge_response.headers().clone();
        let challenge_body_text = challenge_response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        parse_suno_challenge_probe_verified_response(
            challenge_status,
            &challenge_headers,
            &challenge_body_text,
            prepared.execution_plan.missing_challenge_token,
        )?;

        let send_plan = build_suno_generate_plan(
            &prepared.base_url,
            req,
            model,
            prepared.user_tier.as_deref(),
        )?;

        let send_response = self
            .send_plan(&send_plan, prepared.runtime_headers.clone())
            .timeout(prepared.execution_plan.request_timeout)
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let send_status = send_response.status().as_u16();
        let send_headers = send_response.headers().clone();
        let send_body_text = send_response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let (initial_clips, clip_ids) = parse_suno_generation_http_poll_seed(
            send_status,
            &send_headers,
            &send_body_text,
            prepared.execution_plan.missing_challenge_token,
        )?;
        if !prepared.execution_plan.wait_completion || suno::clips_ready(&initial_clips) {
            return self
                .finalize_suno_media_response(
                    req,
                    model,
                    &prepared.prompt,
                    &initial_clips,
                    suno::clips_ready(&initial_clips),
                    None,
                    prepared.execution_plan.request_timeout,
                )
                .await;
        }
        let wait_deadline = std::time::Instant::now() + prepared.execution_plan.wait_timeout;
        let mut latest_clips = initial_clips;

        while std::time::Instant::now() < wait_deadline {
            sleep(prepared.execution_plan.poll_interval).await;

            let response = self
                .send_plan(
                    &build_suno_feed_poll_plan(&prepared.base_url, req.endpoint_kind, &clip_ids),
                    prepared.runtime_headers.clone(),
                )
                .timeout(prepared.execution_plan.request_timeout)
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            let status = response.status().as_u16();
            let headers = response.headers().clone();
            let body_text = response
                .text()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            latest_clips = parse_suno_feed_http_clips_response(
                status,
                &headers,
                &body_text,
                prepared.execution_plan.missing_challenge_token,
            )?;
            if suno::clips_ready(&latest_clips) {
                return self
                    .finalize_suno_media_response(
                        req,
                        model,
                        &prepared.prompt,
                        &latest_clips,
                        true,
                        None,
                        prepared.execution_plan.request_timeout,
                    )
                    .await;
            }
        }

        self.finalize_suno_media_response(
            req,
            model,
            &prepared.prompt,
            &latest_clips,
            false,
            Some("Timed out waiting for Suno clip generation to reach a terminal state."),
            prepared.execution_plan.request_timeout,
        )
        .await
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
        let prepared =
            prepare_suno_execution_context(payload, req, extra_headers, true, self.timeout)?;

        let worker_result = if let Some(result) = self
            .execute_remote_browser_executor(
                "suno",
                provider_account_id,
                req.endpoint_kind,
                build_suno_browser_executor_payload(
                    &prepared.base_url,
                    &prepared.runtime_headers,
                    req,
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
                &req.raw_body,
                prepared.execution_plan.target_asset_kind,
                prepared.execution_plan.wait_completion,
                prepared.execution_plan.wait_timeout,
                prepared.execution_plan.poll_interval,
                prepared.execution_plan.request_timeout,
            )
            .await?
        };

        self.finalize_suno_media_response(
            req,
            model,
            &prepared.prompt,
            &worker_result.clips,
            worker_result.completed,
            worker_result.message.as_deref(),
            prepared.execution_plan.request_timeout,
        )
        .await
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
            EndpointKind::VideosGenerations | EndpointKind::MusicGenerations => {
                build_suno_non_image_generation_response(
                    req.endpoint_kind,
                    model,
                    prompt,
                    clips,
                    completed,
                    message,
                )
            }
            _ => Err(suno::unsupported_media_endpoint_error()),
        }
    }

    async fn execute_suno_browser_worker(
        &self,
        provider: &str,
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

        let mut child = Command::new(
            std::env::var("SUNO_BROWSER_NODE_BIN").unwrap_or_else(|_| "node".to_string()),
        )
        .arg(&script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            suno::browser_worker_spawn_failed_error(
                script_path.as_path(),
                error.to_string().as_str(),
            )
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&stdin_json)
                .await
                .map_err(|error| suno::browser_worker_stdin_error(error.to_string().as_str()))?;
        }

        let output = tokio::time::timeout(timeout, child.wait_with_output())
            .await
            .map_err(|_| suno::browser_worker_timeout_error())?
            .map_err(|error| suno::browser_worker_wait_failed_error(error.to_string().as_str()))?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        parse_suno_browser_worker_verified_output(&stdout, &stderr, provider)
    }
}
