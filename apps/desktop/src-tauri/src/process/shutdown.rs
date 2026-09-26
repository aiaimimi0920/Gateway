use crate::state::{GatewayProcessRuntime, GatewayProcessSnapshot};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};
const GRACEFUL_SHUTDOWN_TIMEOUT_MS: u64 = 4_000;

const GRACEFUL_SHUTDOWN_POLL_INTERVAL_MS: u64 = 200;

pub(crate) fn terminate_process_tree(child: &mut Child) {
    let pid = child.id().to_string();

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = Command::new("taskkill")
            .args(["/PID", pid.as_str(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    #[cfg(not(windows))]
    {
        let _ = Command::new("pkill")
            .args(["-TERM", "-P", pid.as_str()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    let _ = child.kill();
    let _ = child.wait();
}

pub(super) fn build_drain_request(
    client: &reqwest::blocking::Client,
    port: u16,
    management_token: Option<&str>,
) -> reqwest::blocking::RequestBuilder {
    let mut request = client.post(format!(
        "http://127.0.0.1:{port}/v1/internal/gateway/runtime/drain"
    ));
    if let Some(token) = management_token
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        request = request.header("x-management-token", token);
    }
    request.json(&serde_json::json!({"reason": "desktop_stop"}))
}

fn try_graceful_shutdown(
    snapshot: &GatewayProcessSnapshot,
    management_token: Option<&str>,
) -> bool {
    let Some(port) = snapshot.port else {
        return false;
    };

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(client) => client,
        Err(_) => return false,
    };
    build_drain_request(&client, port, management_token)
        .send()
        .map(|response| response.status().is_success() || response.status().is_redirection())
        .unwrap_or(false)
}

fn wait_for_graceful_exit(child: &mut Child) -> Option<ExitStatus> {
    let timeout = Duration::from_millis(GRACEFUL_SHUTDOWN_TIMEOUT_MS);
    let poll_interval = Duration::from_millis(GRACEFUL_SHUTDOWN_POLL_INTERVAL_MS);
    let started_at = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {
                if started_at.elapsed() >= timeout {
                    return None;
                }
                std::thread::sleep(poll_interval);
            }
            Err(_) => return None,
        }
    }
}

pub(super) fn shutdown_state_for_exit(drain_requested: bool, status: &ExitStatus) -> &'static str {
    if drain_requested && status.success() {
        "graceful"
    } else {
        "exited"
    }
}

pub(crate) fn shutdown_process_runtime(runtime: &mut GatewayProcessRuntime) {
    if !runtime.snapshot.running && runtime.child.is_none() {
        runtime.management_token = None;
        return;
    }

    runtime.snapshot.shutdown_state = Some("draining".to_string());
    let management_token = runtime.management_token.clone();
    let drain_requested = try_graceful_shutdown(&runtime.snapshot, management_token.as_deref());
    let mut exit_status = None;
    let mut forced = false;
    if let Some(child) = runtime.child.as_mut() {
        match wait_for_graceful_exit(child) {
            Some(status) => exit_status = Some(status),
            None => {
                terminate_process_tree(child);
                forced = true;
            }
        }
    } else {
        forced = true;
        runtime.snapshot.last_error = Some(
            "Gateway process handle was unavailable during stop; the desktop could not verify a graceful exit"
                .to_string(),
        );
    }

    if forced {
        runtime.snapshot.shutdown_state = Some("forced".to_string());
        if runtime.snapshot.last_error.is_none() {
            runtime.snapshot.last_error = Some(
                "Gateway did not exit cleanly during the graceful shutdown window; the sidecar process tree was force-terminated"
                    .to_string(),
            );
        }
    } else if let Some(status) = exit_status {
        runtime.snapshot.shutdown_state =
            Some(shutdown_state_for_exit(drain_requested, &status).to_string());
        if status.success() {
            runtime.snapshot.last_error = None;
        } else {
            runtime.snapshot.last_error = Some(format!(
                "Gateway sidecar exited during shutdown with nonzero status {status}"
            ));
        }
    }

    runtime.child = None;
    runtime.management_token = None;
    runtime.snapshot.running = false;
    runtime.snapshot.pid = None;
    runtime.snapshot.startup_state = Some("stopped".to_string());
    if runtime.snapshot.shutdown_state.as_deref() == Some("graceful") {
        runtime.snapshot.last_error = None;
    }
}
