use crate::paths::gateway_profile_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;

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

pub fn validate_profile(profile: &GatewayProfile) -> Result<(), String> {
    if profile.name.trim().is_empty() {
        return Err("profile name cannot be empty".to_string());
    }
    if profile.port == 0 {
        return Err("profile port must be greater than 0".to_string());
    }
    let redis_url = profile.gateway_redis_url.trim();
    if redis_url.is_empty() {
        return Err("GATEWAY_REDIS_URL cannot be empty".to_string());
    }
    if !redis_url.starts_with("redis://") && !redis_url.starts_with("rediss://") {
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
    Ok(())
}

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
        package_directory.join("neuro-gateway.exe"),
        package_directory.join("neuro-gateway"),
        package_directory.join("bin").join("neuro-gateway.exe"),
        package_directory.join("bin").join("neuro-gateway"),
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
            candidates.push(
                ancestor
                    .join("target")
                    .join(profile)
                    .join("neuro-gateway.exe"),
            );
            candidates.push(ancestor.join("target").join(profile).join("neuro-gateway"));
        }
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| package_directory.join("neuro-gateway.exe"))
}

pub fn resolve_gateway_sidecar_path() -> Result<PathBuf, String> {
    Ok(resolve_gateway_sidecar_path_from(
        &current_executable_directory()?,
    ))
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

fn inspect_profile_path(
    configured_path: Option<String>,
    resolved_path: PathBuf,
    expected_kind: &str,
    ok_message: &str,
    missing_message: &str,
) -> GatewayProfilePathCheckItem {
    let exists = resolved_path.exists();
    let is_file = resolved_path.is_file();
    let is_dir = resolved_path.is_dir();
    let ok = match expected_kind {
        "file" => is_file,
        "directory" => is_dir,
        _ => exists,
    };
    let message = if ok { ok_message } else { missing_message }.to_string();

    GatewayProfilePathCheckItem {
        configured_path,
        resolved_path: resolved_path.display().to_string(),
        expected_kind: expected_kind.to_string(),
        exists,
        is_file,
        is_dir,
        ok,
        message,
    }
}

fn optional_unconfigured_path_check(
    expected_kind: &str,
    message: &str,
) -> GatewayProfilePathCheckItem {
    GatewayProfilePathCheckItem {
        configured_path: None,
        resolved_path: String::new(),
        expected_kind: expected_kind.to_string(),
        exists: false,
        is_file: false,
        is_dir: false,
        ok: true,
        message: message.to_string(),
    }
}

fn failed_path_check(
    configured_path: Option<String>,
    expected_kind: &str,
    message: String,
) -> GatewayProfilePathCheckItem {
    GatewayProfilePathCheckItem {
        configured_path,
        resolved_path: String::new(),
        expected_kind: expected_kind.to_string(),
        exists: false,
        is_file: false,
        is_dir: false,
        ok: false,
        message,
    }
}

fn parse_dependency_endpoint(
    configured_url: &str,
    accepted_schemes: &[&str],
    default_port: u16,
) -> Result<(String, u16), String> {
    let parsed = reqwest::Url::parse(configured_url)
        .map_err(|_| "configured URL is not valid".to_string())?;
    if !accepted_schemes.contains(&parsed.scheme()) {
        return Err(format!(
            "configured URL must use one of: {}",
            accepted_schemes.join(", ")
        ));
    }
    let host = parsed
        .host_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "configured URL is missing a host".to_string())?;
    Ok((host.to_string(), parsed.port().unwrap_or(default_port)))
}

fn tcp_dependency_check(
    name: &str,
    configured_url: &str,
    accepted_schemes: &[&str],
    default_port: u16,
    required: bool,
    remediation: &str,
) -> GatewayDependencyCheckItem {
    let endpoint = parse_dependency_endpoint(configured_url, accepted_schemes, default_port);
    let (host, port) = match endpoint {
        Ok(endpoint) => endpoint,
        Err(error) => {
            return GatewayDependencyCheckItem {
                name: name.to_string(),
                required,
                configured: true,
                ok: false,
                message: format!("{name} configuration is invalid: {error}. {remediation}"),
            };
        }
    };

    let addresses = (host.as_str(), port).to_socket_addrs();
    let reachable = addresses
        .ok()
        .and_then(|mut values| {
            values
                .any(|address| {
                    TcpStream::connect_timeout(&address, Duration::from_millis(500)).is_ok()
                })
                .then_some(())
        })
        .is_some();

    GatewayDependencyCheckItem {
        name: name.to_string(),
        required,
        configured: true,
        ok: reachable,
        message: if reachable {
            format!("{name} accepted a TCP connection")
        } else {
            format!("{name} is not reachable. {remediation}")
        },
    }
}

fn optional_database_check(profile: &GatewayProfile) -> GatewayDependencyCheckItem {
    let Some(database_url) = profile
        .gateway_database_url
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return GatewayDependencyCheckItem {
            name: "PostgreSQL".to_string(),
            required: false,
            configured: false,
            ok: true,
            message:
                "PostgreSQL is not configured; database-backed Gateway features remain disabled"
                    .to_string(),
        };
    };

    tcp_dependency_check(
        "PostgreSQL",
        database_url,
        &["postgres", "postgresql"],
        5432,
        false,
        "Start PostgreSQL or update GATEWAY_DATABASE_URL.",
    )
}

fn redis_check(profile: &GatewayProfile) -> GatewayDependencyCheckItem {
    tcp_dependency_check(
        "Redis",
        profile.gateway_redis_url.trim(),
        &["redis", "rediss"],
        6379,
        true,
        "Start Redis or update GATEWAY_REDIS_URL.",
    )
}

pub fn preflight_gateway_profile_from(
    profile: &GatewayProfile,
    package_directory: &Path,
) -> GatewayProfilePreflight {
    let configured_working_directory = profile
        .working_directory
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let working_directory_result =
        resolve_profile_working_directory_from(profile, package_directory);
    let working_directory = match &working_directory_result {
        Ok(path) => inspect_profile_path(
            configured_working_directory,
            path.clone(),
            "directory",
            "working directory is available",
            "working directory does not exist or is not a directory",
        ),
        Err(error) => failed_path_check(configured_working_directory, "directory", error.clone()),
    };

    let sidecar_path = resolve_gateway_sidecar_path_from(package_directory);
    let sidecar = inspect_profile_path(
        None,
        sidecar_path,
        "file",
        "Gateway sidecar executable is available",
        "Gateway sidecar executable is missing; place neuro-gateway.exe next to the desktop executable",
    );

    let gateway_routes_file = if let Some(routes_file) = profile
        .gateway_routes_file
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        match resolve_profile_routes_file_from(profile, package_directory, routes_file) {
            Ok(path) => inspect_profile_path(
                Some(routes_file.to_string()),
                path,
                "file",
                "gateway routes file is available",
                "gateway routes file does not exist or is not a file",
            ),
            Err(error) => failed_path_check(Some(routes_file.to_string()), "file", error),
        }
    } else {
        let default_routes = working_directory_result
            .as_ref()
            .ok()
            .map(|path| path.join("routes.yaml"));
        match default_routes {
            Some(path) if path.is_file() => inspect_profile_path(
                None,
                path,
                "file",
                "default routes.yaml is available",
                "default routes.yaml is unavailable",
            ),
            Some(path) => GatewayProfilePathCheckItem {
                configured_path: None,
                resolved_path: path.display().to_string(),
                expected_kind: "file".to_string(),
                exists: path.exists(),
                is_file: path.is_file(),
                is_dir: path.is_dir(),
                ok: true,
                message: "GATEWAY_ROUTES_FILE is not configured and default routes.yaml is absent; Gateway may still load routes from Redis"
                    .to_string(),
            },
            None => optional_unconfigured_path_check(
                "file",
                "GATEWAY_ROUTES_FILE is not configured; resolve the working directory before checking default routes.yaml",
            ),
        }
    };

    let redis = redis_check(profile);
    let database = optional_database_check(profile);
    let mut messages = Vec::new();
    for (ok, message) in [
        (sidecar.ok, sidecar.message.as_str()),
        (working_directory.ok, working_directory.message.as_str()),
        (gateway_routes_file.ok, gateway_routes_file.message.as_str()),
        (redis.ok, redis.message.as_str()),
        (database.ok, database.message.as_str()),
    ] {
        if !ok {
            messages.push(message.to_string());
        }
    }
    let ok =
        sidecar.ok && working_directory.ok && gateway_routes_file.ok && redis.ok && database.ok;

    GatewayProfilePreflight {
        ok,
        sidecar,
        working_directory,
        gateway_routes_file,
        redis,
        database,
        messages,
    }
}

pub fn preflight_gateway_profile(
    profile: &GatewayProfile,
) -> Result<GatewayProfilePreflight, String> {
    let package_directory = current_executable_directory()?;
    Ok(preflight_gateway_profile_from(profile, &package_directory))
}

pub fn preflight_failure_message(preflight: &GatewayProfilePreflight) -> Option<String> {
    (!preflight.ok).then(|| {
        format!(
            "Gateway dependency preflight failed: {}",
            preflight.messages.join("; ")
        )
    })
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

fn sanitize_profile_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("profile name cannot be empty".to_string());
    }

    let mut sanitized = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }
    Ok(sanitized)
}

fn profile_path(name: &str) -> Result<PathBuf, String> {
    let sanitized = sanitize_profile_name(name)?;
    Ok(gateway_profile_dir()?.join(format!("{sanitized}.json")))
}

fn read_profile_from_path(path: &Path) -> Result<GatewayProfile, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read profile {}: {error}", path.display()))?;
    serde_json::from_str::<GatewayProfile>(&text)
        .map_err(|error| format!("failed to parse profile {}: {error}", path.display()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_package_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("neuro-gateway-profile-contract-{nonce}"))
    }

    #[test]
    fn profile_serializes_runtime_contract_and_resolves_package_relative_routes() {
        let package_dir = temporary_package_dir();
        let working_dir = package_dir.join("runtime");
        fs::create_dir_all(&working_dir).expect("create test working directory");
        fs::write(working_dir.join("routes.yaml"), "routes: []\n").expect("write test routes file");

        let profile = GatewayProfile {
            name: "portable".to_string(),
            runtime_role: GatewayRuntimeRole::Standalone,
            gateway_management_token: Some("management-secret".to_string()),
            port: 4200,
            gateway_redis_url: "redis://127.0.0.1:6379".to_string(),
            gateway_database_url: None,
            gateway_routes_file: Some("routes.yaml".to_string()),
            log_level: Some("info".to_string()),
            working_directory: Some("runtime".to_string()),
            extra_env: Vec::new(),
        };

        let resolved_working_dir = resolve_profile_working_directory_from(&profile, &package_dir)
            .expect("resolve working directory");
        let resolved_routes = resolve_profile_routes_file_from(
            &profile,
            &package_dir,
            profile
                .gateway_routes_file
                .as_deref()
                .expect("routes value"),
        )
        .expect("resolve routes file");
        let serialized = serde_json::to_value(&profile).expect("serialize profile");

        assert_eq!(resolved_working_dir, working_dir);
        assert_eq!(resolved_routes, working_dir.join("routes.yaml"));
        assert_eq!(serialized["runtimeRole"], "standalone");
        assert_eq!(serialized["gatewayManagementToken"], "management-secret");

        let _ = fs::remove_dir_all(package_dir);
    }

    #[test]
    fn dev_layout_resolves_headless_binary_from_gateway_target() {
        let package_directory = temporary_package_dir()
            .join("Gateway")
            .join("apps")
            .join("desktop")
            .join("src-tauri")
            .join("target")
            .join("debug");
        let gateway_target = package_directory
            .ancestors()
            .nth(5)
            .expect("Gateway ancestor")
            .join("target")
            .join("debug");
        fs::create_dir_all(&package_directory).expect("create package directory");
        fs::create_dir_all(&gateway_target).expect("create Gateway target directory");
        let expected = gateway_target.join(if cfg!(windows) {
            "neuro-gateway.exe"
        } else {
            "neuro-gateway"
        });
        fs::write(&expected, b"fixture").expect("write headless fixture");

        assert_eq!(
            resolve_gateway_sidecar_path_from(&package_directory),
            expected
        );
        let _ = fs::remove_dir_all(
            package_directory
                .ancestors()
                .nth(5)
                .expect("Gateway ancestor"),
        );
    }
}
