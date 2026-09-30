use super::{
    profile_working_directory_path, resolve_profile_file_path, GatewayProfile,
    DESKTOP_OWNED_ENV_KEYS,
};
use std::collections::HashSet;
pub fn validate_profile(profile: &GatewayProfile) -> Result<(), String> {
    if profile.name.trim().is_empty() {
        return Err("profile name cannot be empty".to_string());
    }
    if profile.port == 0 {
        return Err("profile port must be greater than 0".to_string());
    }
    let redis_url = profile.gateway_redis_url.trim();
    if !profile.local_storage() && redis_url.is_empty() {
        return Err("GATEWAY_REDIS_URL cannot be empty".to_string());
    }
    if !profile.local_storage()
        && !redis_url.starts_with("redis://")
        && !redis_url.starts_with("rediss://")
    {
        return Err("GATEWAY_REDIS_URL must start with redis:// or rediss://".to_string());
    }
    if let Some(database_url) = profile
        .gateway_database_url
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
            return Err(
                "GATEWAY_DATABASE_URL must start with postgres:// or postgresql://".to_string(),
            );
        }
    }
    validate_profile_paths(profile)?;
    validate_extra_env(profile)?;
    if let Some(mode) = profile
        .extra_env
        .iter()
        .find(|entry| entry.key == "GATEWAY_STORAGE_MODE")
    {
        if !matches!(
            mode.value.trim().to_ascii_lowercase().as_str(),
            "local" | "server"
        ) {
            return Err("GATEWAY_STORAGE_MODE must be local or server".into());
        }
    }
    if profile.local_storage() && profile.runtime_role != super::GatewayRuntimeRole::Standalone {
        return Err("Local storage requires standalone runtime role".into());
    }
    if !profile.local_storage()
        && profile
            .extra_env
            .iter()
            .any(|entry| entry.key == "GATEWAY_DESKTOP_MANAGED" && entry.value == "1")
    {
        return Err("Desktop-managed backends require local storage".into());
    }
    Ok(())
}

fn validate_profile_paths(profile: &GatewayProfile) -> Result<(), String> {
    let working_directory = profile_working_directory_path(profile)?;

    if let Some(routes_file) = profile
        .gateway_routes_file
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        let resolved_routes_file = resolve_profile_file_path(profile, routes_file)?;
        if !resolved_routes_file.is_file() {
            return Err(format!(
                "gateway routes file does not exist: {}",
                resolved_routes_file.display()
            ));
        }
    }

    if !working_directory.is_dir() {
        return Err(format!(
            "working directory does not exist or is not a directory: {}",
            working_directory.display()
        ));
    }

    Ok(())
}

fn validate_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn validate_extra_env(profile: &GatewayProfile) -> Result<(), String> {
    let reserved: HashSet<&str> = DESKTOP_OWNED_ENV_KEYS.iter().copied().collect();
    let mut seen = HashSet::new();
    for entry in &profile.extra_env {
        let key = entry.key.trim();
        if key.is_empty() {
            return Err("extra env key cannot be empty".to_string());
        }
        if !validate_env_key(key) {
            return Err(format!(
                "invalid extra env key {key}; use letters, numbers, and underscore, and do not start with a number"
            ));
        }

        let normalized_key = key.to_ascii_uppercase();
        if reserved.contains(normalized_key.as_str()) {
            return Err(format!(
                "reserved extra env key cannot be overridden: {key}"
            ));
        }
        if !seen.insert(normalized_key) {
            return Err(format!("duplicate extra env key: {key}"));
        }
    }

    Ok(())
}
