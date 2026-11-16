//! Reuse the Console's validated paths, writer lock and atomic replace implementation.
use super::Settings;
use crate::console::{ConsoleConfig, RouteConfigPersistence};
use std::io::Read;

fn store(config: &ConsoleConfig) -> Result<RouteConfigPersistence, String> {
    RouteConfigPersistence::new(&config.state_dir, &config.routes_file)
        .map_err(|error| error.to_string())
}

pub(super) fn load(config: &ConsoleConfig) -> Result<Settings, String> {
    let path = config.state_dir.join("console/lan-sharing.json");
    match std::fs::symlink_metadata(&path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Settings { enabled: true })
        }
        Err(error) => return Err(error.to_string()),
    }
    let store = store(config)?;
    let path = store.state_root().join("lan-sharing.json");
    store
        .validate_optional_managed_file(&path)
        .map_err(|error| error.to_string())?;
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Settings { enabled: true })
        }
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > 4096 {
        return Err("LAN sharing settings are too large".into());
    }
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

pub(super) fn save(config: &ConsoleConfig, settings: Settings) -> Result<(), String> {
    let store = store(config)?;
    let guard = store.try_writer_lock().map_err(|error| error.to_string())?;
    store
        .atomic_write_json_locked(
            &guard,
            &store.state_root().join("lan-sharing.json"),
            &settings,
        )
        .map_err(|error| error.to_string())
}
