use crate::paths::gateway_runtime_dir;
use crate::profile::{GatewayProfile, DESKTOP_OWNED_ENV_KEYS};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
pub(super) fn render_child_environment(
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
        "GATEWAY_STORAGE_MODE".into(),
        if profile.local_storage() {
            "local"
        } else {
            "server"
        }
        .into(),
    );
    if !profile.local_storage() {
        environment.insert(
            "GATEWAY_REDIS_URL".to_string(),
            profile.gateway_redis_url.trim().to_string(),
        );
    }
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

pub(super) fn configure_child_environment(
    command: &mut Command,
    environment: BTreeMap<String, String>,
) {
    // The managed local instance must not inherit another deployment's state,
    // credentials or background jobs. Keep OS/PATH variables for native workers.
    if environment
        .get("GATEWAY_DESKTOP_MANAGED")
        .map(String::as_str)
        == Some("1")
    {
        for (key, _) in std::env::vars_os() {
            if key
                .to_string_lossy()
                .to_ascii_uppercase()
                .starts_with("GATEWAY_")
            {
                command.env_remove(key);
            }
        }
    }
    for key in DESKTOP_OWNED_ENV_KEYS {
        command.env_remove(key);
    }
    for (key, value) in environment {
        command.env(key, value);
    }
}

pub(super) fn write_runtime_env_markers(profile: &GatewayProfile) -> Result<(), String> {
    let runtime_dir = gateway_runtime_dir()?;
    let profile_path = runtime_dir.join("last-profile.txt");
    std::fs::write(profile_path, profile.name.as_bytes())
        .map_err(|error| format!("failed to persist last profile marker: {error}"))
}
