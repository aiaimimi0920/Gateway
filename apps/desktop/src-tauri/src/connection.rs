pub(crate) mod authorization_browser;
pub mod chrome;
pub(crate) mod headless;
mod local;
pub(crate) mod panel;
mod panel_request;
pub(crate) mod presentation;
mod settings;
pub(crate) mod window;

use crate::paths::gateway_app_dir;
use crate::process::stop_gateway_sidecar;
use crate::state::GatewayDesktopState;
use settings::{ConnectionMode, ConnectionSettings};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, Webview};

#[derive(Default)]
pub struct ConnectionState {
    busy: Mutex<()>,
    origin: Mutex<Option<String>>,
}

fn require_settings_window(window: &Webview) -> Result<(), String> {
    if !matches!(window.label(), "main" | panel::LABEL) {
        return Err("Connection commands are only available to the local settings window".into());
    }
    Ok(())
}

#[tauri::command]
pub fn load_gateway_connection(window: Webview) -> Result<ConnectionSettings, String> {
    require_settings_window(&window)?;
    let result = gateway_app_dir().and_then(|dir| settings::load(&dir.join("connection.json")));
    if result.is_err() && window.label() == "main" {
        window::show_settings(window.app_handle());
    }
    result
}

#[tauri::command]
pub async fn connect_gateway(
    app: AppHandle,
    window: Webview,
    settings: ConnectionSettings,
) -> Result<String, String> {
    require_settings_window(&window)?;
    let recovery_window = window.label() == "main";
    let fallback = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || connect(&app, settings))
        .await
        .map_err(|error| error.to_string())
        .and_then(|result| result);
    // The settings window stays hidden during automatic startup, but failures must remain actionable.
    if result.is_err() && recovery_window {
        window::show_settings(&fallback);
    }
    result
}

fn connect(app: &AppHandle, settings: ConnectionSettings) -> Result<String, String> {
    let connection = app.state::<ConnectionState>();
    let _guard = connection
        .busy
        .try_lock()
        .map_err(|_| "A Gateway connection is already in progress")?;
    let is_local = settings.mode == ConnectionMode::Local;
    let remote = if is_local {
        None
    } else {
        Some(settings::console_url(&settings.server_url)?)
    };
    let url = if let Some(url) = remote {
        // A failed remote probe must not stop the currently connected local backend.
        window::wait_for_console(&url, false)?;
        url
    } else {
        local::start(app)?
    };
    if is_local {
        window::wait_for_console(&url, true)?;
    }
    settings::save(&gateway_app_dir()?.join("connection.json"), &settings)?;
    let address = url.to_string();
    window::open(app, url)?;
    if !is_local {
        stop_gateway_sidecar(app.state::<GatewayDesktopState>())?;
    }
    Ok(address)
}

pub fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        if crate::desktop_settings::minimize_to_tray(window.app_handle()) {
            api.prevent_close();
            let _ = window.hide();
            return;
        }
        if window.label() == "gateway-console" {
            window.app_handle().exit(0);
        } else if window.label() == "main"
            && window
                .app_handle()
                .get_window("gateway-console")
                .is_some_and(|console| console.is_visible().unwrap_or(false))
        {
            api.prevent_close();
            let _ = window.hide();
        } else {
            window.app_handle().exit(0);
        }
    }
}

#[tauri::command]
pub async fn test_gateway_connection(
    window: Webview,
    settings: ConnectionSettings,
) -> Result<String, String> {
    require_settings_window(&window)?;
    let url = if settings.mode == ConnectionMode::Local {
        settings::console_url("http://127.0.0.1:4200")?
    } else {
        settings::console_url(&settings.server_url)?
    };
    tauri::async_runtime::spawn_blocking(move || {
        window::wait_for_console(&url, false)?;
        Ok(url.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}
