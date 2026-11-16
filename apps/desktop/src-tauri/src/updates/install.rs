//! Switch only after the managed child drains. External gateways are never addressed.
use super::{download, model};
use crate::state::GatewayDesktopState;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};

fn command(path: &Path, root: &Path) -> Result<Command, String> {
    crate::startup_arguments::command(path, root)?;
    let mut command = Command::new(path);
    let package = path.parent().ok_or("Missing package directory")?;
    command
        .arg("--gateway-updated")
        .arg("--gateway-data-dir")
        .arg(root)
        .current_dir(package)
        .env("GATEWAY_RELEASE_PAYLOAD_ROOT", package);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    Ok(command)
}
pub fn redirect() -> Result<bool, String> {
    if std::env::args().any(|value| value == "--gateway-updated") {
        return Ok(false);
    }
    let root = gateway_local_data::selected_root().map_err(|e| e.to_string())?;
    let active = root.join("updates/active.json");
    if !active.exists() {
        return Ok(false);
    }
    let record: serde_json::Value = model::read_json(&active)?;
    let package =
        std::path::PathBuf::from(record["package"].as_str().ok_or("Invalid active update")?);
    let package = package
        .canonicalize()
        .map_err(|_| "Active update package is missing")?;
    let updates = root
        .join("updates")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !package.starts_with(&updates) {
        return Err("Active update is outside the update directory".into());
    }
    let exe = package.join("gateway-ui.exe");
    if exe.canonicalize().ok()
        == std::env::current_exe()
            .ok()
            .and_then(|p| p.canonicalize().ok())
    {
        return Ok(false);
    }
    command(&exe, &root)?
        .spawn()
        .map_err(|_| "Cannot start active Gateway desktop version")?;
    Ok(true)
}
pub fn install(app: &AppHandle, root: &Path, package: &Path) -> Result<(), String> {
    if !model::installed().1 {
        return Err("Automatic installation requires an official desktop release, not a development or hand-test build".into());
    }
    let previous = std::env::current_exe().map_err(|e| e.to_string())?;
    crate::startup_arguments::command(&previous, root)?;
    let parent = package.parent().ok_or("Invalid update staging directory")?;
    if !parent.join("prepared").is_file() {
        return Err("Update package has not been verified".into());
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let script = parent.join(format!("restart-{nonce}.ps1"));
    let request = parent.join(format!("restart-{nonce}.json"));
    let receipt = parent.join(format!("started-{nonce}"));
    fs::write(&script, include_str!("restart.ps1")).map_err(|e| e.to_string())?;
    fs::write(&request, serde_json::to_vec(&serde_json::json!({"pid":std::process::id(),"package":package,"previous":previous,"root":root,"receipt":receipt})).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    // Establish the helper before draining; it cannot act until this attempt is armed.
    let mut helper = download::powershell(&script, &request)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Cannot start desktop update handoff")?;
    let state = app.state::<GatewayDesktopState>();
    let mut runtime = state.process.try_lock().map_err(|_| "Gateway is busy")?;
    if let Some(child) = runtime.child.as_mut() {
        if child.try_wait().map_err(|e| e.to_string())?.is_none() {
            let port = runtime
                .snapshot
                .port
                .ok_or("Managed backend port is unknown")?;
            let client = reqwest::blocking::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|e| e.to_string())?;
            let ready: serde_json::Value = client
                .get(format!("http://127.0.0.1:{port}/readyz"))
                .send()
                .and_then(|r| r.error_for_status())
                .and_then(|r| r.json())
                .map_err(|_| "Cannot confirm managed backend is idle")?;
            if ready["active_requests"]
                .as_u64()
                .is_none_or(|count| count > 1)
            {
                return Err("The managed backend has active requests; update postponed".into());
            }
            let mut request = client.post(format!(
                "http://127.0.0.1:{port}/v1/internal/gateway/runtime/drain"
            ));
            if let Some(token) = &runtime.management_token {
                request = request.header("x-management-token", token);
            }
            request
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|_| "Managed backend did not accept graceful shutdown")?;
            let deadline = Instant::now() + Duration::from_secs(90);
            while runtime
                .child
                .as_mut()
                .ok_or("Managed process disappeared")?
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_none()
            {
                if Instant::now() >= deadline {
                    return Err(
                        "Managed backend is still draining; no process was killed or updated"
                            .into(),
                    );
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            runtime.child = None;
            runtime.snapshot.running = false;
        }
    }
    drop(runtime);
    if helper.try_wait().map_err(|e| e.to_string())?.is_some() {
        return Err(
            "Desktop update helper exited before handoff; restart the local connection".into(),
        );
    }
    fs::write(request.with_extension("armed"), b"armed").map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}
