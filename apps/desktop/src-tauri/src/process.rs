use crate::paths::gateway_log_dir;
use crate::profile::{
    load_profile, preflight_gateway_profile, profile_working_directory_path,
    resolve_gateway_sidecar_path, GatewayProfile,
};
use crate::state::{GatewayDesktopState, GatewayProcessSnapshot};
use std::fs::OpenOptions;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
mod environment;
mod shutdown;
mod startup;
use environment::{
    configure_child_environment, render_child_environment, write_runtime_env_markers,
};
pub(crate) use shutdown::{shutdown_process_runtime, terminate_process_tree};
use startup::{
    startup_preflight_failure_message, tail_log_lines, wait_for_startup_probe,
    STARTUP_LOG_TAIL_LINES,
};
#[cfg(test)]
mod tests;

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

fn ensure_port_available(port: u16) -> Result<(), String> {
    if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
        return Err(format!(
            "port {port} is already in use on 127.0.0.1; choose another Gateway profile port or stop the existing service"
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn start_gateway_sidecar(
    state: tauri::State<'_, GatewayDesktopState>,
    profile_name: String,
) -> Result<GatewayProcessSnapshot, String> {
    let profile = load_profile(profile_name)?;
    let preflight = preflight_gateway_profile(&profile)?;
    if let Some(message) = startup_preflight_failure_message(&profile, &preflight) {
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
