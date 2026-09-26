use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use rquest::{Client, Method};
use serde_json::{json, Value};
use tokio::process::Command;
use tracing::debug;

use super::parsing::{
    parse_gemini_business_capture_output, parse_gemini_canvas_capture_output,
    parse_gemini_web_capture_output, parse_remote_gemini_business_capture_output,
    parse_remote_gemini_canvas_capture_output, parse_remote_gemini_web_capture_output,
    summarize_output,
};
use super::{
    GeminiAuthFamily, GeminiAuthHelperExecutionMode, GeminiAuthSessionControlState,
    GeminiBusinessCaptureOutput, GeminiCanvasCaptureOutput, GeminiWebCaptureOutput,
    RemoteGeminiAuthInvocationError, RemoteGeminiAuthInvocationRequest,
    RemoteGeminiAuthInvocationResponse, GEMINI_AUTH_CONTROL_ROOT, GEMINI_AUTH_HELPER_TIMEOUT_SECS,
    GEMINI_AUTH_HOST_HELPER_START_HINT, GEMINI_AUTH_REMOTE_ENDPOINT_KIND,
    GEMINI_AUTH_REMOTE_EXECUTION_MODE, GEMINI_AUTH_REMOTE_HELPER_TIMEOUT_SECS,
    GEMINI_AUTH_REMOTE_PROVIDER, GEMINI_BUSINESS_CAPTURE_SCRIPT, GEMINI_CANVAS_CAPTURE_SCRIPT,
    GEMINI_WEB_CAPTURE_SCRIPT,
};

pub(super) async fn run_gemini_canvas_capture_worker(
    provider_id: &str,
    target_family: GeminiAuthFamily,
    account_label: Option<String>,
    force_complete_signal_relative_path: Option<&str>,
) -> Result<GeminiCanvasCaptureOutput, String> {
    match current_gemini_auth_helper_execution_mode() {
        GeminiAuthHelperExecutionMode::RemotePreferred => {
            match run_remote_gemini_auth_capture_worker(
                provider_id,
                target_family,
                account_label.clone(),
                force_complete_signal_relative_path,
            )
            .await
            .and_then(parse_remote_gemini_canvas_capture_output)
            {
                Ok(output) => return Ok(output),
                Err(error) => {
                    if !local_headed_browser_launch_supported() {
                        return Err(error);
                    }
                    debug!(
                        provider_id,
                        ?target_family,
                        error = %error,
                        "remote Gemini auth helper failed; falling back to local browser launch"
                    );
                }
            }
        }
        GeminiAuthHelperExecutionMode::HostExecutorRequired => {
            return Err(host_executor_required_message(
                "Gemini Canvas",
                gemini_auth_remote_executor_base_url().as_deref(),
            ));
        }
        GeminiAuthHelperExecutionMode::LocalOnly => {}
    }

    let script_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join(GEMINI_CANVAS_CAPTURE_SCRIPT);
    let node_bin = std::env::var("GEMINI_CANVAS_CAPTURE_NODE_BIN")
        .ok()
        .or_else(|| std::env::var("GEMINI_CANVAS_BROWSER_NODE_BIN").ok())
        .unwrap_or_else(|| "node".to_string());
    let mut command = Command::new(node_bin);
    command
        .arg(&script_path)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("GEMINI_CANVAS_CAPTURE_OUTPUT_MODE", "storage_state")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(label) = account_label
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        command.env("GEMINI_CANVAS_CAPTURE_ACCOUNT_LABEL", label);
    }
    if let Some(relative_path) = force_complete_signal_relative_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        command.env("GEMINI_CANVAS_CAPTURE_FORCE_COMPLETE_FILE", relative_path);
    }

    let output = tokio::time::timeout(
        Duration::from_secs(GEMINI_AUTH_HELPER_TIMEOUT_SECS),
        command.output(),
    )
    .await
    .map_err(|_| "Gemini Canvas auth helper timed out before producing output.".to_string())?
    .map_err(|error| {
        format!(
            "Failed to launch Gemini Canvas auth helper at {}: {error}",
            script_path.display()
        )
    })?;
    parse_gemini_canvas_capture_output(
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
        output.status.success(),
    )
}

pub(super) async fn run_gemini_business_capture_worker(
    provider_id: &str,
) -> Result<GeminiBusinessCaptureOutput, String> {
    match current_gemini_auth_helper_execution_mode() {
        GeminiAuthHelperExecutionMode::RemotePreferred => {
            match run_remote_gemini_auth_capture_worker(
                provider_id,
                GeminiAuthFamily::GeminiBusiness,
                None,
                None,
            )
            .await
            .and_then(parse_remote_gemini_business_capture_output)
            {
                Ok(output) => return Ok(output),
                Err(error) => {
                    if !local_headed_browser_launch_supported() {
                        return Err(error);
                    }
                    debug!(
                        provider_id,
                        error = %error,
                        "remote Gemini Business auth helper failed; falling back to local browser launch"
                    );
                }
            }
        }
        GeminiAuthHelperExecutionMode::HostExecutorRequired => {
            return Err(host_executor_required_message(
                "Gemini Business",
                gemini_auth_remote_executor_base_url().as_deref(),
            ));
        }
        GeminiAuthHelperExecutionMode::LocalOnly => {}
    }

    let script_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join(GEMINI_BUSINESS_CAPTURE_SCRIPT);
    let node_bin = std::env::var("GEMINI_BUSINESS_CAPTURE_NODE_BIN")
        .ok()
        .or_else(|| std::env::var("GEMINI_CANVAS_BROWSER_NODE_BIN").ok())
        .unwrap_or_else(|| "node".to_string());
    let output = tokio::time::timeout(
        Duration::from_secs(GEMINI_AUTH_HELPER_TIMEOUT_SECS),
        Command::new(node_bin)
            .arg(&script_path)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| "Gemini Business auth helper timed out before producing output.".to_string())?
    .map_err(|error| {
        format!(
            "Failed to launch Gemini Business auth helper at {}: {error}",
            script_path.display()
        )
    })?;
    parse_gemini_business_capture_output(
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
        output.status.success(),
    )
}

pub(super) async fn run_gemini_web_capture_worker(
    provider_id: &str,
) -> Result<GeminiWebCaptureOutput, String> {
    match current_gemini_auth_helper_execution_mode() {
        GeminiAuthHelperExecutionMode::RemotePreferred => {
            match run_remote_gemini_auth_capture_worker(
                provider_id,
                GeminiAuthFamily::GeminiWeb,
                None,
                None,
            )
            .await
            .and_then(parse_remote_gemini_web_capture_output)
            {
                Ok(output) => return Ok(output),
                Err(error) => {
                    if !local_headed_browser_launch_supported() {
                        return Err(error);
                    }
                    debug!(
                        provider_id,
                        error = %error,
                        "remote Gemini Web auth helper failed; falling back to local browser launch"
                    );
                }
            }
        }
        GeminiAuthHelperExecutionMode::HostExecutorRequired => {
            return Err(host_executor_required_message(
                "Gemini Web",
                gemini_auth_remote_executor_base_url().as_deref(),
            ));
        }
        GeminiAuthHelperExecutionMode::LocalOnly => {}
    }

    let script_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join(GEMINI_WEB_CAPTURE_SCRIPT);
    let node_bin = std::env::var("GEMINI_WEB_CAPTURE_NODE_BIN")
        .ok()
        .or_else(|| std::env::var("GEMINI_CANVAS_BROWSER_NODE_BIN").ok())
        .unwrap_or_else(|| "node".to_string());
    let output = tokio::time::timeout(
        Duration::from_secs(GEMINI_AUTH_HELPER_TIMEOUT_SECS),
        Command::new(node_bin)
            .arg(&script_path)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| "Gemini Web auth helper timed out before producing output.".to_string())?
    .map_err(|error| {
        format!(
            "Failed to launch Gemini Web auth helper at {}: {error}",
            script_path.display()
        )
    })?;
    parse_gemini_web_capture_output(
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
        output.status.success(),
    )
}

pub(super) async fn run_remote_gemini_auth_capture_worker(
    provider_id: &str,
    target_family: GeminiAuthFamily,
    account_label: Option<String>,
    force_complete_signal_relative_path: Option<&str>,
) -> Result<Value, String> {
    let base_url = gemini_auth_remote_executor_base_url()
        .ok_or_else(|| "Gemini host browser executor is not configured.".to_string())?;
    let client = Client::builder()
        .timeout(Duration::from_secs(GEMINI_AUTH_REMOTE_HELPER_TIMEOUT_SECS))
        .build()
        .map_err(|error| {
            format!("Failed to build the Gemini host browser executor client: {error}")
        })?;
    let mut input = json!({
        "targetFamily": target_family,
    });
    if let Some(label) = account_label
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        input["accountLabel"] = Value::String(label);
    }
    if let Some(relative_path) = force_complete_signal_relative_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        input["forceCompleteSignalRelativePath"] = Value::String(relative_path.to_string());
    }
    let request = RemoteGeminiAuthInvocationRequest {
        provider: GEMINI_AUTH_REMOTE_PROVIDER,
        provider_account_id: provider_id,
        endpoint_kind: GEMINI_AUTH_REMOTE_ENDPOINT_KIND,
        execution_mode: GEMINI_AUTH_REMOTE_EXECUTION_MODE,
        input,
    };
    let url = format!("{base_url}/v1/internal/browser-executor/execute");
    let mut builder = client
        .request(Method::POST, &url)
        .header(rquest::header::CONTENT_TYPE, "application/json")
        .json(&request);
    if let Some(token) = gemini_auth_remote_executor_bearer_token() {
        builder = builder.bearer_auth(token);
    }

    let response = builder.send().await.map_err(|error| {
        format!(
            "Failed to reach the Gemini host browser executor at {url}: {error}. {}",
            GEMINI_AUTH_HOST_HELPER_START_HINT
        )
    })?;
    let status = response.status().as_u16();
    let body_text = response.text().await.unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(format!(
            "Gemini host browser executor at {url} returned status {status}: {}",
            summarize_output(&body_text)
        ));
    }

    let parsed = serde_json::from_str::<RemoteGeminiAuthInvocationResponse>(&body_text).map_err(
        |error| {
            format!(
                "Gemini host browser executor at {url} returned invalid JSON: {error}. payload: {}",
                summarize_output(&body_text)
            )
        },
    )?;
    if parsed.ok {
        return parsed.result.ok_or_else(|| {
            "Gemini host browser executor succeeded but returned no result payload.".to_string()
        });
    }

    Err(format_remote_gemini_auth_invocation_error(
        parsed.status,
        parsed.error,
    ))
}

fn gemini_auth_remote_executor_base_url() -> Option<String> {
    first_non_empty_env_var(&[
        "GATEWAY_GEMINI_AUTH_EXECUTOR_BASE_URL",
        "GATEWAY_BROWSER_EXECUTOR_BASE_URL",
    ])
    .map(|value| value.trim_end_matches('/').to_string())
}

fn gemini_auth_remote_executor_bearer_token() -> Option<String> {
    first_non_empty_env_var(&[
        "GATEWAY_GEMINI_AUTH_EXECUTOR_BEARER_TOKEN",
        "GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN",
    ])
}

pub(super) fn prepare_gemini_auth_session_control_state(
    session_id: &str,
) -> Result<GeminiAuthSessionControlState, String> {
    let control_dir = absolute_control_path(&format!("{GEMINI_AUTH_CONTROL_ROOT}/{session_id}"));
    std::fs::create_dir_all(&control_dir).map_err(|error| {
        format!(
            "Failed to create Gemini auth control directory at {}: {error}",
            control_dir.display()
        )
    })?;
    let complete_signal_relative_path =
        format!("{GEMINI_AUTH_CONTROL_ROOT}/{session_id}/complete.signal");
    let complete_signal_absolute_path =
        absolute_control_path(complete_signal_relative_path.as_str());
    let _ = std::fs::remove_file(complete_signal_absolute_path);
    Ok(GeminiAuthSessionControlState {
        complete_signal_relative_path,
    })
}

pub(super) fn absolute_control_path(relative_path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path)
}

fn first_non_empty_env_var(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn current_gemini_auth_helper_execution_mode() -> GeminiAuthHelperExecutionMode {
    resolve_gemini_auth_helper_execution_mode(
        std::env::consts::OS,
        std::env::var("DISPLAY").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        gemini_auth_remote_executor_base_url().as_deref(),
    )
}

pub(super) fn resolve_gemini_auth_helper_execution_mode(
    platform: &str,
    display: Option<&str>,
    wayland_display: Option<&str>,
    remote_base_url: Option<&str>,
) -> GeminiAuthHelperExecutionMode {
    if let Some(base_url) = remote_base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let _ = base_url;
        return GeminiAuthHelperExecutionMode::RemotePreferred;
    }
    if local_headed_browser_launch_supported_for(platform, display, wayland_display) {
        GeminiAuthHelperExecutionMode::LocalOnly
    } else {
        GeminiAuthHelperExecutionMode::HostExecutorRequired
    }
}

fn local_headed_browser_launch_supported() -> bool {
    local_headed_browser_launch_supported_for(
        std::env::consts::OS,
        std::env::var("DISPLAY").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
    )
}

fn local_headed_browser_launch_supported_for(
    platform: &str,
    display: Option<&str>,
    wayland_display: Option<&str>,
) -> bool {
    if platform == "windows" {
        return true;
    }
    display
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        || wayland_display
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
}

fn host_executor_required_message(surface_label: &str, remote_base_url: Option<&str>) -> String {
    if let Some(base_url) = remote_base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!(
            "{surface_label} manual add could not use the configured host browser executor at {base_url}. {}",
            GEMINI_AUTH_HOST_HELPER_START_HINT
        );
    }
    format!(
        "{surface_label} manual add requires a visible host browser in this Docker-style workflow. Configure GATEWAY_GEMINI_AUTH_EXECUTOR_BASE_URL (for example http://host.docker.internal:42341) and {}",
        GEMINI_AUTH_HOST_HELPER_START_HINT
    )
}

fn format_remote_gemini_auth_invocation_error(
    top_level_status: Option<u16>,
    error: Option<RemoteGeminiAuthInvocationError>,
) -> String {
    let Some(error) = error else {
        return format!(
            "Gemini host browser executor failed without an error payload.{}",
            top_level_status
                .map(|status| format!(" status={status}"))
                .unwrap_or_default()
        );
    };
    let mut message = error
        .message
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Gemini host browser executor failed.")
        .to_string();
    if let Some(code) = error
        .code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        message.push_str(&format!(" [code={code}]"));
    }
    if let Some(status) = error.status.or(top_level_status) {
        message.push_str(&format!(" (status {status})"));
    }
    if let Some(body) = error
        .body
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        message.push_str(&format!(" body: {}", summarize_output(body)));
    }
    message
}
