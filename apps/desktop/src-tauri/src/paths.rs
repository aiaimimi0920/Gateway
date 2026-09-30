use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

// Keep the shared lease alive until this process exits, including external mode.
static DATA: OnceLock<(PathBuf, fs::File)> = OnceLock::new();

fn ensure_dir(path: PathBuf) -> Result<PathBuf, String> {
    fs::create_dir_all(&path)
        .map_err(|error| format!("failed to create directory {}: {error}", path.display()))?;
    Ok(path)
}

pub fn gateway_app_dir() -> Result<PathBuf, String> {
    if let Some((path, _)) = DATA.get() {
        return Ok(path.clone());
    }
    let path = gateway_local_data::selected_root().map_err(|error| error.to_string())?;
    let lease = gateway_local_data::prepare(&path).map_err(|error| error.to_string())?;
    let _ = DATA.set((path.clone(), lease));
    Ok(path)
}

pub fn gateway_profile_dir() -> Result<PathBuf, String> {
    ensure_dir(gateway_app_dir()?.join("profiles"))
}

pub fn gateway_runtime_dir() -> Result<PathBuf, String> {
    ensure_dir(gateway_app_dir()?.join("runtime"))
}

pub fn gateway_log_dir() -> Result<PathBuf, String> {
    ensure_dir(gateway_app_dir()?.join("logs"))
}
