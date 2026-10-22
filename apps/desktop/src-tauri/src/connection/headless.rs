//! Disabled UI is an API-only launch, before constructing Tauri or any WebView.
//! The launcher waits for its backend; no Tauri exit handler owns this child.
use gateway_local_data::startup::StartupConfig;
use std::process::Command;

pub(crate) fn run_if_disabled() -> Result<bool, String> {
    let config_path = StartupConfig::config_path().map_err(|error| error.to_string())?;
    let startup = StartupConfig::from_path(&config_path).map_err(|error| error.to_string())?;
    if startup.enable_management_ui {
        return Ok(false);
    }

    let root = gateway_local_data::selected_root().map_err(|error| error.to_string())?;
    let settings = super::settings::load(&root.join("connection.json"))?;
    if settings.mode == super::settings::ConnectionMode::Existing {
        // Never change an existing-server selection into an implicit local server.
        return Err("Management UI is disabled; the existing remote Gateway remains untouched. Use its API directly.".into());
    }
    let executable = crate::profile::resolve_gateway_sidecar_path()?;
    if !executable.is_file() {
        return Err("Bundled gateway backend was not found".into());
    }
    let package = executable
        .parent()
        .ok_or("Missing Gateway package directory")?;
    let mut command = Command::new(&executable);
    command.current_dir(package);
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GATEWAY_")
        {
            command.env_remove(key);
        }
    }
    command.envs(gateway_local_data::local_environment(&root));
    command
        .env("GATEWAY_CONFIG_FILE", config_path)
        .env("GATEWAY_RUNTIME_ROLE", "standalone")
        .env("GATEWAY_BIND_HOST", "127.0.0.1")
        .env("GATEWAY_DESKTOP_MANAGED", "1")
        .env("GATEWAY_RELEASE_PAYLOAD_ROOT", package);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let status = command
        .status()
        .map_err(|error| format!("Cannot start API-only Gateway: {error}"))?;
    if !status.success() {
        return Err(format!("API-only Gateway exited with {status}"));
    }
    Ok(true)
}
