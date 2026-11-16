//! Updates belong to trusted local settings and operate on official desktop packages only.
mod download;
pub(crate) mod handoff;
pub(crate) mod install;
mod model;
mod pending;
mod releases;

use model::{Preferences, Release};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, Webview};

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    busy: bool,
    release: Option<Release>,
    #[serde(skip)]
    package: Option<PathBuf>,
    downloaded: bool,
    last_checked: u64,
    error: Option<String>,
}
#[derive(Default)]
pub struct Updates {
    operation: Mutex<()>,
    progress: Mutex<Progress>,
    stopped: AtomicBool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutStatus {
    preferences: Preferences,
    current_version: String,
    can_install: bool,
    config_directory: String,
    #[serde(flatten)]
    progress: Progress,
}
fn status(app: &AppHandle) -> Result<AboutStatus, String> {
    let root = crate::paths::gateway_app_dir()?;
    let (current_version, formal) = model::installed();
    Ok(AboutStatus {
        preferences: model::preferences(&root)?,
        current_version,
        can_install: cfg!(windows) && formal,
        config_directory: root.to_string_lossy().into_owned(),
        progress: app
            .state::<Updates>()
            .progress
            .lock()
            .map_err(|_| "Update lock poisoned")?
            .clone(),
    })
}
fn operate(
    app: &AppHandle,
    action: &str,
    preferences: Option<Preferences>,
) -> Result<AboutStatus, String> {
    if action == "status" {
        return status(app);
    }
    let state = app.state::<Updates>();
    let _operation = state
        .operation
        .try_lock()
        .map_err(|_| "Another update operation is active")?;
    let root = crate::paths::gateway_app_dir()?;
    state
        .progress
        .lock()
        .map_err(|_| "Update lock poisoned")?
        .busy = true;
    let result = (|| {
        match action {
            "save" => {
                let prefs = preferences.ok_or("Missing update settings")?;
                if prefs.auto_restart && !model::installed().1 {
                    return Err(
                        "Automatic installation requires an official desktop release".into(),
                    );
                }
                let previous = model::preferences(&root)?;
                model::save(&root, &prefs)?;
                if previous.channel != prefs.channel
                    || previous.download_source != prefs.download_source
                {
                    let mut progress = state.progress.lock().map_err(|_| "Update lock poisoned")?;
                    progress.release = None;
                    progress.package = None;
                    progress.downloaded = false;
                    progress.last_checked = 0;
                }
            }
            "check" => {
                let prefs = model::preferences(&root)?;
                let result = releases::check(&prefs, &model::installed().0);
                let mut progress = state.progress.lock().map_err(|_| "Update lock poisoned")?;
                progress.last_checked = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_secs();
                let release = result?;
                if progress.release.as_ref().map(|r| &r.version)
                    != release.as_ref().map(|r| &r.version)
                {
                    progress.package = None;
                    progress.downloaded = false;
                }
                progress.release = release;
            }
            "download" => {
                let release = state
                    .progress
                    .lock()
                    .map_err(|_| "Update lock poisoned")?
                    .release
                    .clone()
                    .ok_or("Check for updates first")?;
                let prefs = model::preferences(&root)?;
                let package = download::stage(&root, &prefs, &release)?;
                pending::save(&root, &release, &package, &prefs)?;
                let mut progress = state.progress.lock().map_err(|_| "Update lock poisoned")?;
                progress.package = Some(package);
                progress.downloaded = true;
            }
            "install" => {
                let package = state
                    .progress
                    .lock()
                    .map_err(|_| "Update lock poisoned")?
                    .package
                    .clone()
                    .ok_or("Download and verify an update first")?;
                install::install(app, &root, &package)?;
            }
            "openDirectory" => open_directory(&root)?,
            "openProject" => open_link(model::PROJECT)?,
            "openCommunity" => open_link(&format!("{}/issues", model::PROJECT))?,
            "openLicense" => open_link(&format!("{}/blob/main/LICENSE", model::PROJECT))?,
            "openReleases" => open_link(&format!("{}/releases", model::PROJECT))?,
            _ => return Err("Unknown update operation".into()),
        }
        Ok::<(), String>(())
    })();
    {
        let mut progress = state.progress.lock().map_err(|_| "Update lock poisoned")?;
        progress.busy = false;
        progress.error = result.as_ref().err().cloned();
    }
    result?;
    status(app)
}
fn open_directory(path: &std::path::Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe")
            .arg(path)
            .spawn()
            .map_err(|_| "Cannot open configuration directory")?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("Opening the configuration directory is only supported on Windows".into())
    }
}

fn open_link(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = std::process::Command::new("rundll32.exe");
        command
            .arg("url.dll,FileProtocolHandler")
            .creation_flags(0x08000000);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    command
        .arg(url)
        .spawn()
        .map_err(|_| "Cannot open the project website")?;
    Ok(())
}

#[tauri::command]
pub async fn gateway_about_action(
    app: AppHandle,
    window: Webview,
    action: String,
    preferences: Option<Preferences>,
) -> Result<AboutStatus, String> {
    if !matches!(window.label(), "main" | crate::connection::panel::LABEL) {
        return Err("Update operations require the trusted local settings window".into());
    }
    tauri::async_runtime::spawn_blocking(move || operate(&app, &action, preferences))
        .await
        .map_err(|_| "Update task failed")?
}
pub fn start(app: &AppHandle) {
    if let Ok(root) = crate::paths::gateway_app_dir() {
        let result = model::preferences(&root).and_then(|prefs| pending::load(&root, &prefs));
        if let Ok(mut progress) = app.state::<Updates>().progress.lock() {
            match result {
                Ok(Some((release, package))) => {
                    progress.release = Some(release);
                    progress.package = Some(package);
                    progress.downloaded = true;
                }
                Ok(None) => {}
                Err(error) => progress.error = Some(error),
            }
            let result_path = root.join("updates/last-update-result.json");
            if result_path.exists() {
                match model::read_json::<serde_json::Value>(&result_path) {
                    Ok(result) if result["success"] == false => {
                        progress.error = Some(
                            result["message"]
                                .as_str()
                                .unwrap_or("Previous update failed")
                                .to_owned(),
                        );
                    }
                    Err(error) => progress.error = Some(error),
                    _ => {}
                }
            }
        }
    }
    let app = app.clone();
    std::thread::spawn(move || loop {
        if app.state::<Updates>().stopped.load(Ordering::Relaxed) {
            break;
        }
        if let Ok(current) = status(&app) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|v| v.as_secs())
                .unwrap_or(0);
            if current.preferences.automatic
                && now.saturating_sub(current.progress.last_checked)
                    >= u64::from(current.preferences.interval_hours) * 3600
            {
                if let Ok(checked) = operate(&app, "check", None) {
                    if checked.preferences.auto_download
                        && checked.progress.release.is_some()
                        && !checked.progress.downloaded
                    {
                        let _ = operate(&app, "download", None);
                    }
                }
            }
            if let Ok(current) = status(&app) {
                if current.preferences.automatic
                    && current.preferences.auto_restart
                    && current.progress.downloaded
                    && current.can_install
                {
                    let _ = operate(&app, "install", None);
                }
            }
        }
        std::thread::sleep(Duration::from_secs(60));
    });
}
pub fn stop(app: &AppHandle) {
    app.state::<Updates>()
        .stopped
        .store(true, Ordering::Relaxed);
}
