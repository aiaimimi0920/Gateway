use super::GatewayProfile;
use std::path::{Path, PathBuf};
pub fn current_executable_directory() -> Result<PathBuf, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("failed to locate desktop executable: {error}"))?;
    executable
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "desktop executable has no parent directory".to_string())
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

pub fn resolve_profile_working_directory_from(
    profile: &GatewayProfile,
    package_directory: &Path,
) -> Result<PathBuf, String> {
    let configured = profile
        .working_directory
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let resolved = configured
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                package_directory.join(path)
            }
        })
        .unwrap_or_else(|| package_directory.to_path_buf());
    let resolved = normalize_path(resolved);
    if resolved.is_dir() {
        Ok(resolved)
    } else if configured.is_some() {
        Err(format!(
            "working directory does not exist or is not a directory: {}",
            resolved.display()
        ))
    } else {
        Err(format!(
            "desktop package directory is unavailable; configure a working directory explicitly (checked {})",
            resolved.display()
        ))
    }
}

pub fn profile_working_directory_path(profile: &GatewayProfile) -> Result<PathBuf, String> {
    let package_directory = current_executable_directory()?;
    resolve_profile_working_directory_from(profile, &package_directory)
}

pub fn resolve_profile_routes_file_from(
    profile: &GatewayProfile,
    package_directory: &Path,
    value: &str,
) -> Result<PathBuf, String> {
    let working_directory = resolve_profile_working_directory_from(profile, package_directory)?;
    let path = PathBuf::from(value.trim());
    Ok(normalize_path(if path.is_absolute() {
        path
    } else {
        working_directory.join(path)
    }))
}

pub fn resolve_profile_file_path(profile: &GatewayProfile, value: &str) -> Result<PathBuf, String> {
    let package_directory = current_executable_directory()?;
    resolve_profile_routes_file_from(profile, &package_directory, value)
}

pub fn resolve_gateway_sidecar_path_from(package_directory: &Path) -> PathBuf {
    let mut candidates = vec![
        package_directory.join("gateway.exe"),
        package_directory.join("gateway"),
        package_directory.join("bin").join("gateway.exe"),
        package_directory.join("bin").join("gateway"),
    ];

    let target_profiles = if cfg!(debug_assertions) {
        ["debug", "release"]
    } else {
        ["release", "debug"]
    };
    for ancestor in package_directory.ancestors().take(8) {
        let is_gateway_root = ancestor
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.eq_ignore_ascii_case("Gateway"))
            .unwrap_or(false);
        if !is_gateway_root {
            continue;
        }
        for profile in target_profiles {
            candidates.push(ancestor.join("target").join(profile).join("gateway.exe"));
            candidates.push(ancestor.join("target").join(profile).join("gateway"));
        }
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| package_directory.join("gateway.exe"))
}

pub fn resolve_gateway_sidecar_path() -> Result<PathBuf, String> {
    Ok(resolve_gateway_sidecar_path_from(
        &current_executable_directory()?,
    ))
}
