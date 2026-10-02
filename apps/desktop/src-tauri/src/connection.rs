pub(crate) mod authorization_browser;
pub mod chrome;
mod local;
mod settings;
mod window;

use crate::paths::gateway_app_dir;
use crate::process::stop_gateway_sidecar;
use crate::state::GatewayDesktopState;
use settings::{ConnectionMode, ConnectionSettings};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, WebviewWindow};

#[derive(Default)]
pub struct ConnectionState {
    busy: Mutex<()>,
    origin: Mutex<Option<String>>,
}

fn require_settings_window(window: &WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Connection commands are only available to the local settings window".into());
    }
    Ok(())
}

#[tauri::command]
pub fn load_gateway_connection(window: WebviewWindow) -> Result<ConnectionSettings, String> {
    require_settings_window(&window)?;
    settings::load(&gateway_app_dir()?.join("connection.json"))
}

#[tauri::command]
pub async fn connect_gateway(
    app: AppHandle,
    window: WebviewWindow,
    settings: ConnectionSettings,
) -> Result<String, String> {
    require_settings_window(&window)?;
    tauri::async_runtime::spawn_blocking(move || connect(&app, settings))
        .await
        .map_err(|error| error.to_string())?
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
    // Persist the user's choice even on connection failure. Existing mode never
    // falls back to launching a local process, including after application restart.
    settings::save(&gateway_app_dir()?.join("connection.json"), &settings)?;
    let url = if let Some(url) = remote {
        stop_gateway_sidecar(app.state::<GatewayDesktopState>())?;
        url
    } else {
        local::start(app)?
    };
    window::wait_for_console(&url, is_local)?;
    let address = url.to_string();
    window::open(app, url)?;
    Ok(address)
}

pub fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == "gateway-console" {
            window.app_handle().exit(0);
        } else if window.label() == "main"
            && window
                .app_handle()
                .get_webview_window("gateway-console")
                .is_some()
        {
            api.prevent_close();
            let _ = window.hide();
        } else {
            window.app_handle().exit(0);
        }
    }
}
