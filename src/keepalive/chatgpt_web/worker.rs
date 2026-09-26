//! Launch and decode the ChatGPT browser worker using its existing process protocol.
use super::super::{decode_jwt_expiry_iso, request_time_local_browser_worker_blocking_error};
use super::input::chatgpt_web_refresh_input;
use super::types::{
    ChatGptWebRefreshedRuntime, ChatGptWebSessionWorkerExecution, ChatGptWebSessionWorkerOutput,
    ChatGptWebSessionWorkerRelayRequest,
};
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::request_time_browser_policy::RequestTimeBrowserPolicy;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::debug;

const CHATGPT_WEB_REFRESH_TIMEOUT_SECS: u64 = 150;
fn chatgpt_web_session_worker_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("chatgpt-web-session-worker.mjs")
}

pub(super) async fn execute_chatgpt_web_session_worker_internal(
    payload: &ProviderAccountPayload,
    relay_request: Option<ChatGptWebSessionWorkerRelayRequest>,
) -> Result<ChatGptWebSessionWorkerExecution, GatewayError> {
    let provider = "chatgpt_web_reverse_compatible";
    if let Some(error) = request_time_local_browser_worker_blocking_error(
        RequestTimeBrowserPolicy::from_env(),
        provider,
    ) {
        return Err(error);
    }

    let script_path = chatgpt_web_session_worker_script_path();
    if !script_path.exists() {
        return Err(GatewayError::server_error(format!(
            "ChatGPT Web session worker is missing at {}.",
            script_path.display()
        ))
        .with_provider(provider)
        .with_code("chatgpt_web_session_worker_missing"));
    }

    let mut input = chatgpt_web_refresh_input(payload);
    input.relay_request = relay_request;
    let stdin_json = serde_json::to_vec(&input).map_err(|error| {
        GatewayError::server_error(format!(
            "Failed to serialize ChatGPT Web session worker input: {error}"
        ))
        .with_provider(provider)
        .with_code("chatgpt_web_session_worker_input_serialize_failed")
    })?;

    let node_bin = std::env::var("CHATGPT_WEB_BROWSER_NODE_BIN")
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

    let mut command = Command::new(node_bin);
    command
        .arg(&script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(not(target_os = "windows"))]
    {
        // Linux/WSL gateway workers commonly run without a usable X server even
        // when DISPLAY is inherited. Force headless by default so the browser
        // relay path does not die before it can reuse the existing session.
        if std::env::var_os("CHATGPT_WEB_SESSION_WORKER_HEADLESS").is_none() {
            command.env("CHATGPT_WEB_SESSION_WORKER_HEADLESS", "true");
        }
    }
    let mut child = command.spawn().map_err(|error| {
        GatewayError::server_error(format!(
            "Failed to launch ChatGPT Web session worker at {}: {error}",
            script_path.display()
        ))
        .with_provider(provider)
        .with_code("chatgpt_web_session_worker_spawn_failed")
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&stdin_json).await.map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to write ChatGPT Web session worker input: {error}"
            ))
            .with_provider(provider)
            .with_code("chatgpt_web_session_worker_stdin_failed")
        })?;
    }

    let timeout = std::time::Duration::from_secs(CHATGPT_WEB_REFRESH_TIMEOUT_SECS);
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| {
            GatewayError::server_error(
                "ChatGPT Web session worker timed out while refreshing browser session material.",
            )
            .with_provider(provider)
            .with_code("chatgpt_web_session_worker_timeout")
        })?
        .map_err(|error| {
            GatewayError::server_error(format!(
                "ChatGPT Web session worker failed before producing output: {error}"
            ))
            .with_provider(provider)
            .with_code("chatgpt_web_session_worker_wait_failed")
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
            "ChatGPT Web session worker produced no JSON output. stderr: {}",
            if stderr.is_empty() {
                "<empty>"
            } else {
                stderr.as_str()
            }
        ))
        .with_provider(provider)
        .with_code("chatgpt_web_session_worker_empty_output"));
    }

    let worker_output: ChatGptWebSessionWorkerOutput =
        serde_json::from_str(&last_json_line).map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to parse ChatGPT Web session worker output: {error}. stdout: {last_json_line}"
            ))
            .with_provider(provider)
            .with_code("chatgpt_web_session_worker_output_parse_failed")
        })?;

    if !worker_output.ok {
        let message = worker_output
            .error
            .as_ref()
            .and_then(|error| error.message.as_deref())
            .map(str::to_string)
            .or_else(|| (!stderr.is_empty()).then_some(stderr.clone()))
            .unwrap_or_else(|| {
                "ChatGPT Web session worker returned ok=false while refreshing an existing session."
                    .to_string()
            });
        let mut error = GatewayError::server_error(message)
            .with_provider(provider)
            .with_code(
                worker_output
                    .error
                    .as_ref()
                    .and_then(|error| error.code.clone())
                    .unwrap_or_else(|| "chatgpt_web_session_worker_failed".to_string()),
            );
        if let Some(status) = worker_output.error.as_ref().and_then(|error| error.status) {
            error.http_status = Some(status);
        }
        return Err(error);
    }

    let cookie_header = worker_output
        .cookie_header
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let api_key = worker_output
        .auth_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| cookie_header.as_ref().map(|_| String::new()))
        .or_else(|| {
            let api_key = payload.api_key.trim();
            (!api_key.is_empty()).then_some(api_key.to_string())
        })
        .ok_or_else(|| {
            GatewayError::server_error(
                "ChatGPT Web session worker succeeded without returning any usable auth token, and the existing provider payload also had no apiKey to reuse.",
            )
            .with_provider(provider)
            .with_code("chatgpt_web_session_worker_missing_auth_token")
        })?;

    if let Some(path) = worker_output.credential_file.as_deref() {
        debug!(
            credential_file = %path,
            "ChatGPT Web session worker wrote refreshed credential file"
        );
    }

    Ok(ChatGptWebSessionWorkerExecution {
        refreshed: ChatGptWebRefreshedRuntime {
            api_key,
            expires_at: worker_output
                .expires_at
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| decode_jwt_expiry_iso(Some(&payload.api_key))),
            refresh_token: None,
            id_token: None,
            cookie_header,
            device_id: worker_output
                .device_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            session_id: worker_output
                .session_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            client_version: worker_output
                .client_version
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            client_build_number: worker_output
                .client_build_number
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            user_agent: worker_output
                .user_agent
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            language: worker_output
                .language
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            language_code: worker_output
                .language_code
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            timezone: worker_output
                .timezone
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            chatgpt_pow_sources: worker_output
                .chatgpt_pow_sources
                .filter(|items| !items.is_empty()),
            chatgpt_pow_data_build: worker_output
                .chatgpt_pow_data_build
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            account_name: worker_output
                .account_name
                .as_deref()
                .or_else(|| {
                    worker_output
                        .auth_probe
                        .as_ref()
                        .and_then(|probe| probe.email.as_deref())
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            credential_material_key: worker_output
                .credential_material_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        },
        relay_response: worker_output.relay_response,
    })
}

pub(in crate::keepalive) async fn execute_chatgpt_web_session_worker(
    payload: &ProviderAccountPayload,
) -> Result<ChatGptWebRefreshedRuntime, GatewayError> {
    Ok(execute_chatgpt_web_session_worker_internal(payload, None)
        .await?
        .refreshed)
}
