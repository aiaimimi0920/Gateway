use super::StartupProbeResult;
use crate::logs::read_log_tail_lines;
use crate::profile::{GatewayProfile, GatewayProfilePreflight, GatewayRuntimeRole};
use std::path::Path;
use std::time::{Duration, Instant};
const STARTUP_PROBE_TIMEOUT_MS: u64 = 8_000;

const STARTUP_PROBE_POLL_INTERVAL_MS: u64 = 250;

const STARTUP_HTTP_TIMEOUT_MS: u64 = 350;

pub(super) const STARTUP_LOG_TAIL_LINES: usize = 24;

fn standalone_startup_can_use_yaml_without_redis(
    preflight: &GatewayProfilePreflight,
    runtime_role: GatewayRuntimeRole,
) -> bool {
    matches!(runtime_role, GatewayRuntimeRole::Standalone)
        && preflight.sidecar.ok
        && preflight.working_directory.ok
        && preflight.gateway_routes_file.is_file
}

pub(super) fn startup_preflight_failure_message(
    profile: &GatewayProfile,
    preflight: &GatewayProfilePreflight,
) -> Option<String> {
    let mut messages = Vec::new();

    for (ok, message) in [
        (preflight.sidecar.ok, preflight.sidecar.message.as_str()),
        (
            preflight.working_directory.ok,
            preflight.working_directory.message.as_str(),
        ),
        (
            preflight.gateway_routes_file.ok,
            preflight.gateway_routes_file.message.as_str(),
        ),
        (preflight.database.ok, preflight.database.message.as_str()),
    ] {
        if !ok {
            messages.push(message.to_string());
        }
    }

    if !preflight.redis.ok
        && !standalone_startup_can_use_yaml_without_redis(preflight, profile.runtime_role)
    {
        messages.push(preflight.redis.message.clone());
    }

    (!messages.is_empty()).then(|| {
        format!(
            "Gateway dependency preflight failed: {}",
            messages.join("; ")
        )
    })
}

pub(super) fn tail_log_lines(path: &Path, max_lines: usize) -> Vec<String> {
    read_log_tail_lines(path, max_lines).unwrap_or_default()
}

fn probe_gateway_endpoint(client: &reqwest::blocking::Client, port: u16, path: &str) -> bool {
    client
        .get(format!("http://127.0.0.1:{port}{path}"))
        .send()
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

pub(super) fn wait_for_startup_probe(
    child: &mut std::process::Child,
    port: u16,
    log_path: &Path,
) -> StartupProbeResult {
    let timeout = Duration::from_millis(STARTUP_PROBE_TIMEOUT_MS);
    let poll_interval = Duration::from_millis(STARTUP_PROBE_POLL_INTERVAL_MS);
    let started_at = Instant::now();
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(STARTUP_HTTP_TIMEOUT_MS))
        .build()
        .ok();
    let mut last_error = None;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return StartupProbeResult {
                    running: false,
                    startup_state: "exited".to_string(),
                    last_error: Some(format!(
                        "Gateway sidecar exited during startup with status {status}"
                    )),
                    recent_log_lines: tail_log_lines(log_path, STARTUP_LOG_TAIL_LINES),
                };
            }
            Ok(None) => {}
            Err(error) => {
                return StartupProbeResult {
                    running: false,
                    startup_state: "unknown".to_string(),
                    last_error: Some(format!(
                        "failed to inspect Gateway sidecar during startup: {error}"
                    )),
                    recent_log_lines: tail_log_lines(log_path, STARTUP_LOG_TAIL_LINES),
                };
            }
        }

        if let Some(client) = &client {
            let health_ok = probe_gateway_endpoint(client, port, "/healthz");
            let ready_ok = probe_gateway_endpoint(client, port, "/readyz");
            if health_ok || ready_ok {
                return StartupProbeResult {
                    running: true,
                    startup_state: if ready_ok { "ready" } else { "healthy" }.to_string(),
                    last_error: if ready_ok {
                        None
                    } else {
                        Some(
                            "Gateway process is healthy, but /readyz is still failing; optional runtime dependencies may be unavailable"
                                .to_string(),
                        )
                    },
                    recent_log_lines: tail_log_lines(log_path, STARTUP_LOG_TAIL_LINES),
                };
            }
            last_error = Some(
                "Gateway process is running but /healthz and /readyz are not responding yet"
                    .to_string(),
            );
        }

        if started_at.elapsed() >= timeout {
            return StartupProbeResult {
                running: true,
                startup_state: "starting".to_string(),
                last_error: last_error.or_else(|| {
                    Some(
                        "Gateway sidecar is still starting after the startup probe window"
                            .to_string(),
                    )
                }),
                recent_log_lines: tail_log_lines(log_path, STARTUP_LOG_TAIL_LINES),
            };
        }

        std::thread::sleep(poll_interval);
    }
}
