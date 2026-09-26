use super::dependencies::{optional_database_check, redis_check};
use super::{
    resolve_gateway_sidecar_path_from, resolve_profile_routes_file_from,
    resolve_profile_working_directory_from, GatewayProfile, GatewayProfilePathCheckItem,
    GatewayProfilePreflight,
};
use std::path::{Path, PathBuf};
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
        "Gateway sidecar executable is missing; place gateway.exe next to the desktop executable",
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
