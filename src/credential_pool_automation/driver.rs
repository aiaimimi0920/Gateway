use super::registry::resolve_allowlisted_script_path;
use super::{
    CredentialAutomationDriver, CredentialAutomationDriverTransport,
    CredentialPoolAutomationConfig, DriverRequest, DriverResponse,
};
use futures::StreamExt;
use rquest::Method;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

mod script_io;
mod script_path;

#[cfg(test)]
mod http_contract;
#[cfg(test)]
mod script_contract;

const MAX_DRIVER_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

pub(super) async fn execute_driver(
    config: &CredentialPoolAutomationConfig,
    driver: &CredentialAutomationDriver,
    request: &DriverRequest,
) -> anyhow::Result<DriverResponse> {
    let timeout = Duration::from_secs(
        driver
            .timeout_secs
            .unwrap_or(config.default_timeout_secs)
            .clamp(1, 900),
    );
    let bytes = match &driver.transport {
        CredentialAutomationDriverTransport::Script { script } => {
            execute_script_driver(config, script, request, timeout).await?
        }
        CredentialAutomationDriverTransport::Http {
            endpoint,
            secret_env,
        } => execute_http_driver(endpoint, secret_env.as_deref(), request, timeout).await?,
    };
    if bytes.len() > MAX_DRIVER_OUTPUT_BYTES {
        anyhow::bail!("credential automation driver response exceeded the size limit");
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("credential automation driver returned invalid JSON"))
}

async fn execute_script_driver(
    config: &CredentialPoolAutomationConfig,
    script: &str,
    request: &DriverRequest,
    timeout: Duration,
) -> anyhow::Result<Vec<u8>> {
    let script_path = resolve_allowlisted_script_path(config, script)?;
    let extension = script_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut command = match extension.as_str() {
        "ps1" => {
            #[cfg(target_os = "windows")]
            let mut command = Command::new("powershell");
            #[cfg(not(target_os = "windows"))]
            let mut command = Command::new("pwsh");
            command.args(["-NoProfile", "-NonInteractive"]);
            #[cfg(target_os = "windows")]
            command.args(["-ExecutionPolicy", "Bypass"]);
            command.arg("-File");
            command.arg(&script_path);
            command
        }
        "js" | "mjs" | "cjs" => {
            let mut command = Command::new("node");
            command.arg(script_path::node_entry(&script_path)?);
            command
        }
        "py" => {
            #[cfg(target_os = "windows")]
            let mut command = Command::new("python");
            #[cfg(not(target_os = "windows"))]
            let mut command = Command::new("python3");
            command.arg(&script_path);
            command
        }
        "exe" => Command::new(&script_path),
        _ => anyhow::bail!("unsupported automation script extension"),
    };
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|_| anyhow::anyhow!("failed to start credential automation script"))?;
    let request_bytes = serde_json::to_vec(request)?;
    script_io::collect_output(&mut child, &request_bytes, timeout).await
}

async fn execute_http_driver(
    endpoint: &str,
    secret_env: Option<&str>,
    request: &DriverRequest,
    timeout: Duration,
) -> anyhow::Result<Vec<u8>> {
    let client = rquest::Client::builder().timeout(timeout).build()?;
    let mut builder = client.request(Method::POST, endpoint).json(request);
    if let Some(secret_env) = secret_env {
        let token = std::env::var(secret_env)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("credential automation HTTP secret is unavailable"))?;
        builder = builder.bearer_auth(token);
    }
    let response = builder
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("credential automation HTTP request failed"))?;
    if !response.status().is_success() {
        anyhow::bail!("credential automation HTTP endpoint returned a non-success status");
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_DRIVER_OUTPUT_BYTES as u64)
    {
        anyhow::bail!("credential automation driver response exceeded the size limit");
    }
    // Count decoded chunks as well as checking the advertised length.
    let stream = response.bytes_stream();
    futures::pin_mut!(stream);
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|_| anyhow::anyhow!("failed to read credential automation HTTP response"))?;
        if chunk.len() > MAX_DRIVER_OUTPUT_BYTES.saturating_sub(bytes.len()) {
            anyhow::bail!("credential automation driver response exceeded the size limit");
        }
        bytes
            .try_reserve(chunk.len())
            .map_err(|_| anyhow::anyhow!("failed to read credential automation HTTP response"))?;
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
