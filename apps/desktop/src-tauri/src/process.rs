use crate::logs::read_log_tail_lines;
use crate::paths::{gateway_log_dir, gateway_runtime_dir};
use crate::profile::{
    load_profile, preflight_failure_message, preflight_gateway_profile,
    profile_working_directory_path, resolve_gateway_sidecar_path, GatewayProfile,
    DESKTOP_OWNED_ENV_KEYS,
};
use crate::state::{GatewayDesktopState, GatewayProcessRuntime, GatewayProcessSnapshot};
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const GRACEFUL_SHUTDOWN_TIMEOUT_MS: u64 = 4_000;
const GRACEFUL_SHUTDOWN_POLL_INTERVAL_MS: u64 = 200;
const STARTUP_PROBE_TIMEOUT_MS: u64 = 8_000;
const STARTUP_PROBE_POLL_INTERVAL_MS: u64 = 250;
const STARTUP_HTTP_TIMEOUT_MS: u64 = 350;
const STARTUP_LOG_TAIL_LINES: usize = 24;

struct StartupProbeResult {
    running: bool,
    startup_state: String,
    last_error: Option<String>,
    recent_log_lines: Vec<String>,
}

fn now_iso_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("{seconds}")
}

fn gateway_binary_path() -> Result<std::path::PathBuf, String> {
    resolve_gateway_sidecar_path()
}

fn profile_working_directory(profile: &GatewayProfile) -> Result<std::path::PathBuf, String> {
    profile_working_directory_path(profile)
}

fn render_child_environment(
    profile: &GatewayProfile,
    working_directory: &Path,
) -> Result<BTreeMap<String, String>, String> {
    let mut environment = BTreeMap::new();
    environment.insert("PORT".to_string(), profile.port.to_string());
    environment.insert(
        "GATEWAY_RUNTIME_ROLE".to_string(),
        profile.runtime_role.as_str().to_string(),
    );
    environment.insert(
        "GATEWAY_REDIS_URL".to_string(),
        profile.gateway_redis_url.trim().to_string(),
    );
    if let Some(database_url) = profile
        .gateway_database_url
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        environment.insert("GATEWAY_DATABASE_URL".to_string(), database_url.to_string());
    }
    if let Some(routes_file) = profile
        .gateway_routes_file
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        let route_path = Path::new(routes_file);
        let resolved = if route_path.is_absolute() {
            route_path.to_path_buf()
        } else {
            working_directory.join(route_path)
        };
        environment.insert(
            "GATEWAY_ROUTES_FILE".to_string(),
            resolved.to_string_lossy().into_owned(),
        );
    }
    if let Some(token) = profile
        .gateway_management_token
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        environment.insert("GATEWAY_MANAGEMENT_TOKEN".to_string(), token.to_string());
    }
    if let Some(log_level) = profile
        .log_level
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        environment.insert("RUST_LOG".to_string(), log_level.to_string());
    }
    for entry in &profile.extra_env {
        let key = entry.key.trim();
        if !key.is_empty() {
            environment.insert(key.to_string(), entry.value.clone());
        }
    }
    Ok(environment)
}

fn configure_child_environment(command: &mut Command, environment: BTreeMap<String, String>) {
    for key in DESKTOP_OWNED_ENV_KEYS {
        command.env_remove(key);
    }
    for (key, value) in environment {
        command.env(key, value);
    }
}

fn write_runtime_env_markers(profile: &GatewayProfile) -> Result<(), String> {
    let runtime_dir = gateway_runtime_dir()?;
    let profile_path = runtime_dir.join("last-profile.txt");
    std::fs::write(profile_path, profile.name.as_bytes())
        .map_err(|error| format!("failed to persist last profile marker: {error}"))
}

fn ensure_port_available(port: u16) -> Result<(), String> {
    if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
        return Err(format!(
            "port {port} is already in use on 127.0.0.1; choose another Gateway profile port or stop the existing service"
        ));
    }
    Ok(())
}

fn tail_log_lines(path: &Path, max_lines: usize) -> Vec<String> {
    read_log_tail_lines(path, max_lines).unwrap_or_default()
}

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

fn probe_gateway_endpoint(client: &reqwest::blocking::Client, port: u16, path: &str) -> bool {
    client
        .get(format!("http://127.0.0.1:{port}{path}"))
        .send()
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

fn wait_for_startup_probe(
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
                    last_error: None,
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

fn build_drain_request(
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

fn shutdown_state_for_exit(drain_requested: bool, status: &ExitStatus) -> &'static str {
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

#[tauri::command]
pub fn start_gateway_sidecar(
    state: tauri::State<'_, GatewayDesktopState>,
    profile_name: String,
) -> Result<GatewayProcessSnapshot, String> {
    let profile = load_profile(profile_name)?;
    let preflight = preflight_gateway_profile(&profile)?;
    if let Some(message) = preflight_failure_message(&preflight) {
        return Err(message);
    }
    let binary_path = gateway_binary_path()?;
    if !binary_path.exists() {
        return Err(format!(
            "gateway binary not found: {}",
            binary_path.display()
        ));
    }

    let mut guard = state
        .process
        .lock()
        .map_err(|_| "gateway desktop process lock poisoned".to_string())?;
    if guard.snapshot.running {
        return Ok(guard.snapshot.clone());
    }
    ensure_port_available(profile.port)?;

    let log_dir = gateway_log_dir()?;
    let started_at = now_iso_string();
    let log_path = log_dir.join(format!(
        "gateway-{}-{}.log",
        profile.name.replace(' ', "_"),
        started_at
    ));
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|error| {
            format!(
                "failed to create gateway log {}: {error}",
                log_path.display()
            )
        })?;
    let stderr = stdout
        .try_clone()
        .map_err(|error| format!("failed to clone gateway log handle: {error}"))?;

    let working_directory = profile_working_directory(&profile)?;
    let child_environment = render_child_environment(&profile, &working_directory)?;
    let mut command = Command::new(&binary_path);
    command
        .current_dir(&working_directory)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    configure_child_environment(&mut command, child_environment);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to launch gateway sidecar: {error}"))?;
    if let Err(error) = write_runtime_env_markers(&profile) {
        terminate_process_tree(&mut child);
        return Err(error);
    }
    let startup_probe = wait_for_startup_probe(&mut child, profile.port, &log_path);
    let child_id = child.id();
    if !startup_probe.running {
        terminate_process_tree(&mut child);
    }

    let snapshot = GatewayProcessSnapshot {
        running: startup_probe.running,
        pid: if startup_probe.running {
            Some(child_id)
        } else {
            None
        },
        port: Some(profile.port),
        profile_name: Some(profile.name),
        log_path: Some(log_path.display().to_string()),
        started_at: Some(started_at),
        startup_state: Some(startup_probe.startup_state),
        shutdown_state: None,
        last_error: startup_probe.last_error,
        recent_log_lines: startup_probe.recent_log_lines,
    };
    guard.child = if snapshot.running { Some(child) } else { None };
    guard.management_token = if snapshot.running {
        profile.gateway_management_token.clone()
    } else {
        None
    };
    guard.snapshot = snapshot.clone();
    Ok(snapshot)
}

#[tauri::command]
pub fn stop_gateway_sidecar(
    state: tauri::State<'_, GatewayDesktopState>,
) -> Result<GatewayProcessSnapshot, String> {
    let mut guard = state
        .process
        .lock()
        .map_err(|_| "gateway desktop process lock poisoned".to_string())?;
    if !guard.snapshot.running && guard.child.is_none() {
        return Ok(guard.snapshot.clone());
    }

    shutdown_process_runtime(&mut guard);
    Ok(guard.snapshot.clone())
}

#[tauri::command]
pub fn get_gateway_process_snapshot(
    state: tauri::State<'_, GatewayDesktopState>,
) -> Result<GatewayProcessSnapshot, String> {
    let mut guard = state
        .process
        .lock()
        .map_err(|_| "gateway desktop process lock poisoned".to_string())?;

    let mut child_id = None;
    let mut child_finished = false;
    let mut child_status = None;
    if let Some(child) = guard.child.as_mut() {
        child_id = Some(child.id());
        match child.try_wait() {
            Ok(Some(status)) => {
                child_finished = true;
                child_status = Some(status);
            }
            Ok(None) => {}
            Err(_) => {}
        }
    }

    if child_finished {
        guard.child = None;
        guard.management_token = None;
        guard.snapshot.running = false;
        guard.snapshot.pid = None;
        guard.snapshot.startup_state = Some("exited".to_string());
        guard.snapshot.shutdown_state = Some("exited".to_string());
        guard.snapshot.last_error = child_status.and_then(|status| {
            (!status.success()).then(|| format!("Gateway sidecar exited with status {status}"))
        });
        guard.snapshot.recent_log_lines = guard
            .snapshot
            .log_path
            .as_ref()
            .map(|path| tail_log_lines(Path::new(path), STARTUP_LOG_TAIL_LINES))
            .unwrap_or_default();
    } else if let Some(pid) = child_id {
        guard.snapshot.running = true;
        guard.snapshot.pid = Some(pid);
    }

    Ok(guard.snapshot.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::GatewayRuntimeRole;

    fn portable_profile() -> GatewayProfile {
        GatewayProfile {
            name: "portable".to_string(),
            runtime_role: GatewayRuntimeRole::Standalone,
            gateway_management_token: Some("management-secret".to_string()),
            port: 4200,
            gateway_redis_url: "redis://127.0.0.1:6379".to_string(),
            gateway_database_url: None,
            gateway_routes_file: Some("routes.yaml".to_string()),
            log_level: Some("info".to_string()),
            working_directory: None,
            extra_env: Vec::new(),
        }
    }

    #[test]
    fn child_environment_contains_runtime_role_token_and_resolved_routes() {
        let profile = portable_profile();
        let working_directory = std::env::temp_dir().join("gateway-process-contract");
        let environment = render_child_environment(&profile, &working_directory)
            .expect("render child environment");

        assert_eq!(
            environment.get("GATEWAY_RUNTIME_ROLE"),
            Some(&"standalone".to_string())
        );
        assert_eq!(
            environment.get("GATEWAY_MANAGEMENT_TOKEN"),
            Some(&"management-secret".to_string())
        );
        assert_eq!(
            environment.get("GATEWAY_ROUTES_FILE"),
            Some(&working_directory.join("routes.yaml").display().to_string())
        );
    }

    #[test]
    fn empty_optional_profile_values_remove_inherited_desktop_environment() {
        let mut profile = portable_profile();
        profile.gateway_database_url = None;
        profile.gateway_routes_file = None;
        profile.gateway_management_token = None;
        profile.log_level = None;
        let environment = render_child_environment(&profile, &std::env::temp_dir())
            .expect("render child environment");
        let mut command = Command::new(if cfg!(windows) { "cmd" } else { "sh" });
        configure_child_environment(&mut command, environment);
        let configured: BTreeMap<_, _> = command
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect();

        for key in [
            "GATEWAY_DATABASE_URL",
            "DATABASE_URL",
            "GATEWAY_ROUTES_FILE",
            "GATEWAY_MANAGEMENT_TOKEN",
            "RUST_LOG",
        ] {
            assert_eq!(configured.get(key), Some(&None), "{key} must be removed");
        }
        assert_eq!(
            configured.get("GATEWAY_REDIS_URL"),
            Some(&Some("redis://127.0.0.1:6379".to_string()))
        );
    }

    #[test]
    fn drain_request_uses_management_token_header() {
        let client = reqwest::blocking::Client::new();
        let request = build_drain_request(&client, 4200, Some("management-secret"))
            .build()
            .expect("build drain request");
        assert_eq!(
            request
                .headers()
                .get("x-management-token")
                .and_then(|value| value.to_str().ok()),
            Some("management-secret")
        );
    }

    #[test]
    fn nonzero_child_exit_never_counts_as_graceful() {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/C", "exit", "7"]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", "exit 7"]);
            command
        };
        let mut child = command.spawn().expect("spawn exit-status fixture");
        let status = child.wait().expect("wait exit-status fixture");
        assert!(!status.success());
        assert_eq!(shutdown_state_for_exit(true, &status), "exited");
    }

    #[cfg(windows)]
    #[test]
    fn terminate_process_tree_kills_windows_grandchildren() {
        use std::fs;
        use std::thread;
        use std::time::{Duration, SystemTime, UNIX_EPOCH};

        fn process_exists(pid: u32) -> bool {
            Command::new("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
                .unwrap_or(false)
        }

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after unix epoch")
            .as_nanos();
        let pid_file = std::env::temp_dir().join(format!("gateway-tree-{nonce}.pid"));
        let escaped_path = pid_file.display().to_string().replace('\'', "''");
        let script = format!(
            "$child = Start-Process -FilePath $env:ComSpec -ArgumentList '/c','ping 127.0.0.1 -n 60 > nul' -PassThru -WindowStyle Hidden; [IO.File]::WriteAllText('{escaped_path}', $child.Id.ToString()); Start-Sleep -Seconds 60"
        );
        let mut parent = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .spawn()
            .expect("spawn process-tree fixture");

        let started_at = Instant::now();
        let child_pid = loop {
            if let Ok(contents) = std::fs::read_to_string(&pid_file) {
                if let Ok(pid) = contents.trim().parse::<u32>() {
                    break pid;
                }
            }
            assert!(started_at.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(50));
        };
        assert!(process_exists(child_pid));

        terminate_process_tree(&mut parent);
        assert!(!process_exists(child_pid));
        let _ = fs::remove_file(pid_file);
    }
}
