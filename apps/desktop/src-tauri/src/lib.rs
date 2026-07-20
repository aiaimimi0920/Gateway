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
        app_name: "Neuro Gateway".to_string(),
        theme_family: "NeuroTerminal".to_string(),
        gateway_mode: "headless-first".to_string(),
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(GatewayDesktopState::default())
        .invoke_handler(tauri::generate_handler![
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Neuro Gateway desktop");
}
