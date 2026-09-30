use crate::paths::gateway_app_dir;
use crate::process::{get_gateway_process_snapshot, start_profile};
use crate::profile::{
    current_executable_directory, GatewayEnvEntry, GatewayProfile, GatewayRuntimeRole,
};
use crate::state::GatewayDesktopState;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::TcpListener;
use std::path::Path;
use tauri::{AppHandle, Manager, Url};

// The managed instance never edits the package's routes or an existing deployment.
fn prepare_profile(root: &Path, package: &Path, port: u16) -> Result<GatewayProfile, String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    let routes = root.join("routes.yaml");
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&routes)
    {
        Ok(mut file) => file
            .write_all(b"providers: []\nmodel_routes: []\n")
            .map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(format!("Cannot initialize local routes: {error}")),
    }
    let mut extra_env: Vec<GatewayEnvEntry> = [
        ("GATEWAY_DESKTOP_MANAGED", "1".to_string()),
        ("GATEWAY_BIND_HOST", "127.0.0.1".to_string()),
        (
            "GATEWAY_RELEASE_PAYLOAD_ROOT",
            package.to_string_lossy().into_owned(),
        ),
        ("GATEWAY_CONSOLE_REMOTE_ACCESS", "false".to_string()),
        (
            "GATEWAY_CONSOLE_REDIS_NAMESPACE",
            "desktop-local".to_string(),
        ),
    ]
    .into_iter()
    .map(|(key, value)| GatewayEnvEntry {
        key: key.into(),
        value,
    })
    .collect();
    let data_root = root.parent().ok_or("Missing local data root")?;
    extra_env.extend(
        gateway_local_data::local_environment(data_root)
            .into_iter()
            // Routes are supplied by the typed profile field, not extra_env.
            .filter(|(key, _)| *key != "GATEWAY_ROUTES_FILE")
            .map(|(key, value)| GatewayEnvEntry {
                key: key.into(),
                value,
            }),
    );
    Ok(GatewayProfile {
        name: "desktop-managed".into(),
        runtime_role: GatewayRuntimeRole::Standalone,
        // Runtime preserves an existing administrator or supplies the installation default.
        gateway_management_token: None,
        port,
        gateway_redis_url: String::new(),
        gateway_database_url: None,
        gateway_routes_file: Some(routes.to_string_lossy().into_owned()),
        log_level: Some("info".into()),
        working_directory: Some(package.to_string_lossy().into_owned()),
        extra_env,
    })
}

pub fn start(app: &AppHandle) -> Result<Url, String> {
    let state = app.state::<GatewayDesktopState>();
    let existing = get_gateway_process_snapshot(state.clone())?;
    if existing.running {
        return super::settings::console_url(&format!(
            "http://127.0.0.1:{}",
            existing.port.ok_or("Missing local port")?
        ));
    }
    // A busy default port belongs to somebody else; never adopt or stop it.
    let reservation = TcpListener::bind(("127.0.0.1", 4200))
        .or_else(|_| TcpListener::bind(("127.0.0.1", 0)))
        .map_err(|error| format!("Cannot allocate a local Gateway port: {error}"))?;
    let port = reservation
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let profile = prepare_profile(
        &gateway_app_dir()?.join("local"),
        &current_executable_directory()?,
        port,
    )?;
    drop(reservation);
    let snapshot = start_profile(&state, profile)?;
    if !snapshot.running {
        return Err(snapshot
            .last_error
            .unwrap_or_else(|| "Local Gateway failed to start".into()));
    }
    super::settings::console_url(&format!("http://127.0.0.1:{port}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_run_preserves_existing_routes_and_keeps_state_outside_package() {
        let root = std::env::temp_dir().join(format!(
            "gateway-local-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let package = root.join("package");
        std::fs::create_dir_all(&package).unwrap();
        let local = root.join("user/local");
        let profile = prepare_profile(&local, &package, 43210).unwrap();
        assert_eq!(profile.port, 43210);
        assert!(profile.gateway_database_url.is_none());
        assert!(profile.gateway_redis_url.is_empty());
        assert!(profile.local_storage());
        crate::profile::validate_profile(&profile).unwrap();
        assert_eq!(
            std::fs::read_to_string(local.join("routes.yaml")).unwrap(),
            "providers: []\nmodel_routes: []\n"
        );
        std::fs::write(local.join("routes.yaml"), "preserve-user-routes").unwrap();
        prepare_profile(&local, &package, 43211).unwrap();
        assert_eq!(
            std::fs::read_to_string(local.join("routes.yaml")).unwrap(),
            "preserve-user-routes"
        );
        assert!(!package.join("routes.yaml").exists());
        assert!(profile
            .extra_env
            .iter()
            .any(|entry| entry.key == "GATEWAY_BIND_HOST" && entry.value == "127.0.0.1"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
