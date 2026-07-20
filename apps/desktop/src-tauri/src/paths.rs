use std::fs;
use std::path::PathBuf;

const APP_DIR_NAME: &str = "NeuroGatewayDesktop";

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var(name)
        .ok()
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn local_appdata_root() -> Result<PathBuf, String> {
    env_dir("LOCALAPPDATA")
        .or_else(|| env_dir("APPDATA"))
        .or_else(|| env_dir("TEMP"))
        .or_else(|| env_dir("TMP"))
        .or_else(|| Some(std::env::temp_dir()))
        .ok_or_else(|| "unable to resolve local app data root".to_string())
}

fn ensure_dir(path: PathBuf) -> Result<PathBuf, String> {
    fs::create_dir_all(&path)
        .map_err(|error| format!("failed to create directory {}: {error}", path.display()))?;
    Ok(path)
}

pub fn gateway_app_dir() -> Result<PathBuf, String> {
    ensure_dir(local_appdata_root()?.join(APP_DIR_NAME))
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
