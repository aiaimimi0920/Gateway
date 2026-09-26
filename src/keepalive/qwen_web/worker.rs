//! Qwen browser process execution and decoding of the existing worker protocol.
use super::super::request_time_local_browser_worker_blocking_error;
use super::input::qwen_web_refresh_input;
use super::types::{QwenWebRefreshedRuntime, QwenWebSessionWorkerOutput};
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::request_time_browser_policy::RequestTimeBrowserPolicy;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::debug;

const QWEN_WEB_REFRESH_TIMEOUT_SECS: u64 = 120;
fn qwen_web_session_worker_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("qwen-web-session-worker.mjs")
}

pub(in crate::keepalive) async fn execute_qwen_web_session_worker(
    payload: &ProviderAccountPayload,
    model: &str,
) -> Result<QwenWebRefreshedRuntime, GatewayError> {
    let provider = "qwen_web_compatible";
    if let Some(error) = request_time_local_browser_worker_blocking_error(
        RequestTimeBrowserPolicy::from_env(),
        provider,
    ) {
        return Err(error);
    }

    let script_path = qwen_web_session_worker_script_path();
    if !script_path.exists() {
        return Err(GatewayError::server_error(format!(
            "Qwen Web session worker is missing at {}.",
            script_path.display()
        ))
        .with_provider(provider)
        .with_code("qwen_web_session_worker_missing"));
    }

    let input = qwen_web_refresh_input(payload, model);
    let stdin_json = serde_json::to_vec(&input).map_err(|error| {
        GatewayError::server_error(format!(
            "Failed to serialize Qwen Web session worker input: {error}"
        ))
        .with_provider(provider)
        .with_code("qwen_web_session_worker_input_serialize_failed")
    })?;

    let node_bin = std::env::var("QWEN_WEB_BROWSER_NODE_BIN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("PRODUCER_BROWSER_NODE_BIN")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| "node".to_string());

    let mut child = Command::new(node_bin)
        .arg(&script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to launch Qwen Web session worker at {}: {error}",
                script_path.display()
            ))
            .with_provider(provider)
            .with_code("qwen_web_session_worker_spawn_failed")
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&stdin_json).await.map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to write Qwen Web session worker input: {error}"
            ))
            .with_provider(provider)
            .with_code("qwen_web_session_worker_stdin_failed")
        })?;
    }

    let timeout = std::time::Duration::from_secs(QWEN_WEB_REFRESH_TIMEOUT_SECS);
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| {
            GatewayError::server_error(
                "Qwen Web session worker timed out while refreshing browser session material.",
            )
            .with_provider(provider)
            .with_code("qwen_web_session_worker_timeout")
        })?
        .map_err(|error| {
            GatewayError::server_error(format!(
                "Qwen Web session worker failed before producing output: {error}"
            ))
            .with_provider(provider)
            .with_code("qwen_web_session_worker_wait_failed")
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let last_json_line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_string();
    if last_json_line.is_empty() {
        return Err(GatewayError::server_error(format!(
            "Qwen Web session worker produced no JSON output. stderr: {}",
            if stderr.is_empty() {
                "<empty>"
            } else {
                stderr.as_str()
            }
        ))
        .with_provider(provider)
        .with_code("qwen_web_session_worker_empty_output"));
    }

    let worker_output: QwenWebSessionWorkerOutput =
        serde_json::from_str(&last_json_line).map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to parse Qwen Web session worker output: {error}. stdout: {last_json_line}"
            ))
            .with_provider(provider)
            .with_code("qwen_web_session_worker_output_parse_failed")
        })?;

    if !worker_output.ok {
        let message = worker_output
            .error
            .as_ref()
            .and_then(|error| error.message.as_deref())
            .map(str::to_string)
            .or_else(|| (!stderr.is_empty()).then_some(stderr.clone()))
            .unwrap_or_else(|| {
                "Qwen Web session worker returned ok=false while refreshing an existing session."
                    .to_string()
            });
        let mut error = GatewayError::server_error(message)
            .with_provider(provider)
            .with_code(
                worker_output
                    .error
                    .as_ref()
                    .and_then(|error| error.code.clone())
                    .unwrap_or_else(|| "qwen_web_session_worker_failed".to_string()),
            );
        if let Some(status) = worker_output.error.as_ref().and_then(|error| error.status) {
            error.http_status = Some(status);
        }
        return Err(error);
    }

    let api_key = worker_output
        .auth_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error(
                "Qwen Web session worker succeeded without returning an auth token. Qwen Web refresh only works against an existing signed-in browser/session source; the gateway does not register or sign in accounts here.",
            )
            .with_provider(provider)
            .with_code("qwen_web_session_worker_missing_auth_token")
        })?
        .to_string();

    let account_name = worker_output
        .auth_probe
        .as_ref()
        .and_then(|probe| probe.email.as_deref().or(probe.user_id.as_deref()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let credential_material_key = worker_output
        .auth_probe
        .as_ref()
        .and_then(|probe| probe.user_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("qwen-web-user:{value}"));

    if let Some(path) = worker_output.credential_file.as_deref() {
        debug!(credential_file = %path, "Qwen Web session worker wrote refreshed credential file");
    }

    Ok(QwenWebRefreshedRuntime {
        api_key,
        expires_at: worker_output
            .expires_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        cookie_header: worker_output
            .cookie_header
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        account_name,
        selected_display_model: worker_output
            .selected_display_model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        credential_material_key,
    })
}
