use crate::paths::gateway_profile_dir;
use serde::{Deserialize, Serialize};
use std::fs;
mod dependencies;
mod paths;
mod preflight;
mod storage;
mod validation;
pub use paths::{
    current_executable_directory, profile_working_directory_path, resolve_gateway_sidecar_path,
    resolve_gateway_sidecar_path_from, resolve_profile_file_path, resolve_profile_routes_file_from,
    resolve_profile_working_directory_from,
};
pub use preflight::preflight_gateway_profile_from;
use storage::{profile_path, read_profile_from_path};
pub use validation::validate_profile;
#[cfg(test)]
mod tests;

pub(crate) const DESKTOP_OWNED_ENV_KEYS: &[&str] = &[
    "PORT",
    "GATEWAY_REDIS_URL",
    "GATEWAY_DATABASE_URL",
    "DATABASE_URL",
    "GATEWAY_ROUTES_FILE",
    "GATEWAY_RUNTIME_ROLE",
    "GATEWAY_MANAGEMENT_TOKEN",
    "RUST_LOG",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GatewayRuntimeRole {
    Splitter,
    Worker,
    Standalone,
}

impl Default for GatewayRuntimeRole {
    fn default() -> Self {
        // Keep compatibility with the headless runtime's historical default.
        Self::Splitter
    }
}

impl GatewayRuntimeRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Splitter => "splitter",
            Self::Worker => "worker",
            Self::Standalone => "standalone",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayEnvEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProfile {
    pub name: String,
    #[serde(default)]
    pub runtime_role: GatewayRuntimeRole,
    #[serde(default)]
    pub gateway_management_token: Option<String>,
    pub port: u16,
    pub gateway_redis_url: String,
    pub gateway_database_url: Option<String>,
    pub gateway_routes_file: Option<String>,
    pub log_level: Option<String>,
    pub working_directory: Option<String>,
    pub extra_env: Vec<GatewayEnvEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayDependencyCheckItem {
    pub name: String,
    pub required: bool,
    pub configured: bool,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProfilePreflight {
    pub ok: bool,
    pub sidecar: GatewayProfilePathCheckItem,
    pub working_directory: GatewayProfilePathCheckItem,
    pub gateway_routes_file: GatewayProfilePathCheckItem,
    pub redis: GatewayDependencyCheckItem,
    pub database: GatewayDependencyCheckItem,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProfilePathCheckItem {
    pub configured_path: Option<String>,
    pub resolved_path: String,
    pub expected_kind: String,
    pub exists: bool,
    pub is_file: bool,
    pub is_dir: bool,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProfilePathCheck {
    pub working_directory: GatewayProfilePathCheckItem,
    pub gateway_routes_file: GatewayProfilePathCheckItem,
    pub sidecar: GatewayProfilePathCheckItem,
    pub redis: GatewayDependencyCheckItem,
    pub database: GatewayDependencyCheckItem,
    pub preflight_ok: bool,
    pub preflight_messages: Vec<String>,
}

pub fn preflight_gateway_profile(
    profile: &GatewayProfile,
) -> Result<GatewayProfilePreflight, String> {
    let package_directory = current_executable_directory()?;
    Ok(preflight_gateway_profile_from(profile, &package_directory))
}

#[tauri::command]
pub fn list_profiles() -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for entry in fs::read_dir(gateway_profile_dir()?)
        .map_err(|error| format!("failed to list profiles: {error}"))?
    {
        let entry = entry.map_err(|error| format!("failed to inspect profile entry: {error}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }

        match read_profile_from_path(&path) {
            Ok(profile) => names.push(profile.name),
            Err(_) => {
                if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
    }

    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn load_profile(name: String) -> Result<GatewayProfile, String> {
    let path = profile_path(&name)?;
    let profile = read_profile_from_path(&path)?;
    validate_profile(&profile)?;
    Ok(profile)
}

#[tauri::command]
pub fn save_profile(profile: GatewayProfile) -> Result<(), String> {
    validate_profile(&profile)?;
    let path = profile_path(&profile.name)?;
    let payload = serde_json::to_string_pretty(&profile)
        .map_err(|error| format!("failed to serialize profile {}: {error}", profile.name))?;
    fs::write(&path, payload)
        .map_err(|error| format!("failed to write profile {}: {error}", path.display()))
}

#[tauri::command]
pub fn check_gateway_profile_paths(
    profile: GatewayProfile,
) -> Result<GatewayProfilePathCheck, String> {
    let preflight = preflight_gateway_profile(&profile)?;

    Ok(GatewayProfilePathCheck {
        working_directory: preflight.working_directory,
        gateway_routes_file: preflight.gateway_routes_file,
        sidecar: preflight.sidecar,
        redis: preflight.redis,
        database: preflight.database,
        preflight_ok: preflight.ok,
        preflight_messages: preflight.messages,
    })
}

#[tauri::command]
pub fn delete_profile(name: String) -> Result<(), String> {
    let path = profile_path(&name)?;
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path)
        .map_err(|error| format!("failed to delete profile {}: {error}", path.display()))
}
