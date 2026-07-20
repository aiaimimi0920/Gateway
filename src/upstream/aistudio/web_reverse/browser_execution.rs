use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use base64::Engine;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::sleep;
use tracing::info;

use crate::error::{classify_upstream_error, GatewayError};
use crate::protocol::aistudio_web;
use crate::protocol::canonical::EndpointKind;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::aistudio::common::request_plan::build_generate_content_request_plan;
use crate::upstream::aistudio::common::runtime_material::AIStudioWebReverseRuntimeMaterial;
use crate::upstream::aistudio::common::types::AIStudioBrowserWorkerResult;
use crate::upstream::aistudio_web as aistudio_web_reverse_modular;
use crate::upstream::browser_worker_runtime_helpers::aistudio_browser_worker_script_path;
use crate::upstream::client::UpstreamClient;

impl UpstreamClient {
    pub(crate) async fn execute_aistudio_browser_text_request(
        &self,
        remote_provider_account_id: &str,
        payload: &ProviderAccountPayload,
        config: &aistudio_web::AIStudioWebConfig,
        endpoint_kind: EndpointKind,
        model: &str,
        request_body: &serde_json::Value,
    ) -> Result<String, GatewayError> {
        let request_plan = build_generate_content_request_plan(
            &payload.base_url,
            model,
            HashMap::from([("content-type".to_string(), "application/json".to_string())]),
            request_body.clone(),
        );
        let request_spec = request_plan.to_browser_request_spec();
        self.execute_aistudio_browser_request(
            remote_provider_account_id,
            payload,
            config,
            endpoint_kind,
            &request_spec,
        )
        .await
    }

    pub(crate) async fn execute_aistudio_browser_request(
        &self,
        remote_provider_account_id: &str,
        payload: &ProviderAccountPayload,
        config: &aistudio_web::AIStudioWebConfig,
        endpoint_kind: EndpointKind,
        request_spec: &aistudio_web::AIStudioBrowserRequestSpec,
    ) -> Result<String, GatewayError> {
        let provider = aistudio_web::AISTUDIO_WEB_REVERSE_ADAPTER;
        let browser_worker_timeout = std::env::var("AISTUDIO_BROWSER_WORKER_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(420));
        let request_timeout = self.timeout.max(browser_worker_timeout);
        let result = if let Some(result) = self
            .execute_remote_browser_executor(
                aistudio_web::AISTUDIO_BROWSER_EXECUTOR_PROVIDER_KEY,
                remote_provider_account_id,
                endpoint_kind,
                aistudio_web_reverse_modular::build_browser_executor_invocation_input(
                    payload,
                    request_spec,
                    request_timeout,
                )?,
            )
            .await?
        {
            serde_json::from_value::<AIStudioBrowserWorkerResult>(result).map_err(|error| {
                GatewayError::server_error(format!(
                    "Failed to parse remote AI Studio browser worker result: {error}"
                ))
                .with_provider(provider)
                .with_code("aistudio_remote_result_parse_failed")
            })?
        } else {
            self.execute_aistudio_browser_worker(config, request_spec, request_timeout)
                .await?
        };
        if result.ok {
            if let Some(body_text) = result.body_text {
                info!(
                    provider = %provider,
                    endpoint_kind = ?endpoint_kind,
                    body_len = body_text.len(),
                    "aistudio browser request returned inline body_text"
                );
                return Ok(body_text);
            }
            if let Some(body_file_path) = result.body_file_path.as_deref() {
                let body_text = tokio::fs::read_to_string(body_file_path)
                    .await
                    .map_err(|error| {
                        GatewayError::server_error(format!(
                            "AI Studio Web reverse could not read browser worker body file at {}: {error}",
                            body_file_path
                        ))
                        .with_provider(provider)
                        .with_code("aistudio_web_reverse_body_file_read_failed")
                    })?;
                info!(
                    provider = %provider,
                    endpoint_kind = ?endpoint_kind,
                    body_file_path = %body_file_path,
                    body_len = body_text.len(),
                    "aistudio browser request loaded body_file_path"
                );
                let _ = tokio::fs::remove_file(body_file_path).await;
                if let Some(parent) = Path::new(body_file_path).parent() {
                    let _ = tokio::fs::remove_dir_all(parent).await;
                }
                return Ok(body_text);
            }
            if let Some(body_base64) = result.body_base64 {
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(body_base64)
                    .map_err(|error| {
                        GatewayError::server_error(format!(
                            "AI Studio Web reverse returned invalid base64 response body: {error}"
                        ))
                        .with_provider(provider)
                        .with_code("aistudio_web_reverse_invalid_base64_body")
                    })?;
                info!(
                    provider = %provider,
                    endpoint_kind = ?endpoint_kind,
                    body_len = decoded.len(),
                    "aistudio browser request decoded base64 body"
                );
                return Ok(String::from_utf8_lossy(&decoded).to_string());
            }
            return Err(GatewayError::server_error(
                "AI Studio Web reverse browser worker completed without a response body.",
            )
            .with_provider(provider)
            .with_code("aistudio_web_reverse_missing_body"));
        }

        let status = result
            .error
            .as_ref()
            .and_then(|error| error.status)
            .or(result.status)
            .unwrap_or(500);
        let body_text = result
            .error
            .as_ref()
            .and_then(|error| error.body.clone())
            .unwrap_or_default();
        let mut gateway_error = classify_upstream_error(status, &body_text, Some(provider));
        if let Some(worker_error) = result.error {
            if let Some(code) = worker_error.code {
                gateway_error.code = Some(code);
            }
            if let Some(message) = worker_error.message {
                gateway_error.message = message;
            }
            if gateway_error.http_status.is_none() {
                gateway_error.http_status = Some(status);
            }
        }
        Err(gateway_error)
    }

    pub(crate) async fn execute_aistudio_browser_worker(
        &self,
        config: &aistudio_web::AIStudioWebConfig,
        request_spec: &aistudio_web::AIStudioBrowserRequestSpec,
        timeout: Duration,
    ) -> Result<AIStudioBrowserWorkerResult, GatewayError> {
        let provider = aistudio_web::AISTUDIO_WEB_REVERSE_ADAPTER;
        let material = AIStudioWebReverseRuntimeMaterial::from(config);
        let input = material
            .to_browser_worker_input(request_spec, timeout)
            .map_err(|error| error.with_provider(provider))?;
        let stdin_json = serde_json::to_vec(&input).map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to serialize AI Studio browser worker input: {error}"
            ))
            .with_provider(provider)
            .with_code("aistudio_browser_worker_input_serialize_failed")
        })?;

        let script_path = aistudio_browser_worker_script_path();
        let mut child = Command::new(
            std::env::var("AISTUDIO_BROWSER_NODE_BIN").unwrap_or_else(|_| "node".to_string()),
        )
        .arg(&script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to launch AI Studio browser worker at {}: {error}",
                script_path.display()
            ))
            .with_provider(provider)
            .with_code("aistudio_browser_worker_spawn_failed")
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&stdin_json).await.map_err(|error| {
                GatewayError::server_error(format!(
                    "Failed to write AI Studio browser worker input: {error}"
                ))
                .with_provider(provider)
                .with_code("aistudio_browser_worker_stdin_failed")
            })?;
        }

        let result_file_path = input.result_file_path.clone().ok_or_else(|| {
            GatewayError::server_error(
                "AI Studio browser worker result file path was not configured.",
            )
            .with_provider(provider)
            .with_code("aistudio_browser_worker_missing_result_file_path")
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            GatewayError::server_error("AI Studio browser worker launched without a stdout pipe.")
                .with_provider(provider)
                .with_code("aistudio_browser_worker_missing_stdout")
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            GatewayError::server_error("AI Studio browser worker launched without a stderr pipe.")
                .with_provider(provider)
                .with_code("aistudio_browser_worker_missing_stderr")
        })?;

        let stderr_task = tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut reader = BufReader::new(stderr);
            let mut bytes = Vec::new();
            let _ = reader.read_to_end(&mut bytes).await;
            String::from_utf8_lossy(&bytes).trim().to_string()
        });

        let started = Instant::now();
        let mut exit_observed_at: Option<Instant> = None;
        loop {
            match tokio::fs::read_to_string(&result_file_path).await {
                Ok(serialized) => {
                    let _ = tokio::fs::remove_file(&result_file_path).await;
                    let _ = child.start_kill();
                    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
                    return serde_json::from_str(serialized.trim()).map_err(|error| {
                        GatewayError::server_error(format!(
                            "Failed to parse AI Studio browser worker result file {}: {error}. body: {}",
                            result_file_path,
                            serialized.trim()
                        ))
                        .with_provider(provider)
                        .with_code("aistudio_browser_worker_result_file_parse_failed")
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    let _ = child.start_kill();
                    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
                    return Err(GatewayError::server_error(format!(
                        "Failed to read AI Studio browser worker result file {}: {error}",
                        result_file_path
                    ))
                    .with_provider(provider)
                    .with_code("aistudio_browser_worker_result_file_read_failed"));
                }
            }

            if let Some(observed_at) = exit_observed_at {
                if observed_at.elapsed() >= Duration::from_secs(2) {
                    break;
                }
            } else if child
                .try_wait()
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "Failed to probe AI Studio browser worker exit status: {error}"
                    ))
                    .with_provider(provider)
                    .with_code("aistudio_browser_worker_try_wait_failed")
                })?
                .is_some()
            {
                exit_observed_at = Some(Instant::now());
            }

            if started.elapsed() >= timeout {
                let _ = child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
                return Err(GatewayError::server_error(
                    "AI Studio browser worker timed out before producing output.",
                )
                .with_provider(provider)
                .with_code("aistudio_browser_worker_timeout"));
            }

            sleep(Duration::from_millis(50)).await;
        }

        let mut stdout_reader = BufReader::new(stdout);
        let mut stdout_line = String::new();
        let remaining = timeout.saturating_sub(started.elapsed());
        let bytes_read = tokio::time::timeout(remaining, stdout_reader.read_line(&mut stdout_line))
            .await
            .map_err(|_| {
                GatewayError::server_error(
                    "AI Studio browser worker timed out before producing output.",
                )
                .with_provider(provider)
                .with_code("aistudio_browser_worker_timeout")
            })?
            .map_err(|error| {
                GatewayError::server_error(format!(
                    "Failed to read AI Studio browser worker stdout: {error}"
                ))
                .with_provider(provider)
                .with_code("aistudio_browser_worker_stdout_read_failed")
            })?;

        let stdout = stdout_line.trim().to_string();
        let stderr = tokio::time::timeout(Duration::from_secs(2), stderr_task)
            .await
            .ok()
            .and_then(|result| result.ok())
            .unwrap_or_default();

        if bytes_read == 0 {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
            return Err(GatewayError::server_error(format!(
                "AI Studio browser worker exited without JSON output. stderr: {}",
                if stderr.is_empty() {
                    "<empty>"
                } else {
                    stderr.as_str()
                }
            ))
            .with_provider(provider)
            .with_code("aistudio_browser_worker_empty_output"));
        }

        let _ = child.start_kill();
        let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
        if stdout.is_empty() {
            return Err(GatewayError::server_error(format!(
                "AI Studio browser worker did not return JSON output. stderr: {}",
                if stderr.is_empty() {
                    "<empty>"
                } else {
                    stderr.as_str()
                }
            ))
            .with_provider(provider)
            .with_code("aistudio_browser_worker_empty_output"));
        }

        serde_json::from_str(&stdout).map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to parse AI Studio browser worker output: {error}. stdout: {stdout}"
            ))
            .with_provider(provider)
            .with_code("aistudio_browser_worker_output_parse_failed")
        })
    }
}
