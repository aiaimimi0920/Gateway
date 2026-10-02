mod connection;
mod logs;
mod paths;
mod process;
mod profile;
mod state;

use serde::Serialize;
use state::GatewayDesktopState;

use crate::logs::{open_gateway_log_directory, read_gateway_log_tail};
use crate::process::{get_gateway_process_snapshot, start_gateway_sidecar, stop_gateway_sidecar};
use crate::profile::{
    check_gateway_profile_paths, delete_profile, list_profiles, load_profile, save_profile,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayUiRuntimeInfo {
    pub app_name: String,
    pub theme_family: String,
    pub gateway_mode: String,
}

#[tauri::command]
fn get_gateway_ui_runtime_info() -> GatewayUiRuntimeInfo {
    GatewayUiRuntimeInfo {
        app_name: "Gateway UI".to_string(),
        theme_family: "NeuroTerminal".to_string(),
        gateway_mode: "headless-first".to_string(),
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(GatewayDesktopState::default())
        .manage(connection::ConnectionState::default())
        .manage(connection::authorization_browser::AuthorizationBrowser::default())
        .setup(|app| {
            let data = paths::gateway_app_dir().map_err(std::io::Error::other)?;
            let chrome_app = app.handle().clone();
            tauri::WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?
                .data_directory(data.join("webview"))
                .initialization_script(connection::chrome::INITIALIZATION_SCRIPT)
                .on_navigation(move |url| {
                    !connection::chrome::handle_navigation(&chrome_app, "main", url)
                })
                .build()?;
            Ok(())
        })
        .on_window_event(connection::handle_window_event)
        .invoke_handler(|invoke| {
            // Server content must never reach application commands, including legacy commands.
            if invoke.message.webview_ref().label() != "main" {
                invoke
                    .resolver
                    .reject("Native commands are restricted to connection settings");
                return true;
            }
            let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
                connection::load_gateway_connection,
                connection::connect_gateway,
                get_gateway_ui_runtime_info,
                list_profiles,
                load_profile,
                save_profile,
                delete_profile,
                check_gateway_profile_paths,
                start_gateway_sidecar,
                stop_gateway_sidecar,
                get_gateway_process_snapshot,
                open_gateway_log_directory,
                read_gateway_log_tail,
            ];
            handler(invoke)
        })
        .build(tauri::generate_context!())
        .expect("error while building Gateway UI desktop")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                use tauri::Manager;
                app.state::<connection::authorization_browser::AuthorizationBrowser>()
                    .stop();
                let _ = stop_gateway_sidecar(app.state::<GatewayDesktopState>());
            }
        });
}
