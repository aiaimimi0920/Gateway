//! Backend-owned log settings and exports, shared by browser and desktop consoles.
mod export;
mod files;
pub mod logging;
mod redaction;
mod retention;
#[cfg(test)]
mod tests;

use super::ConsoleConfig;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf, sync::Mutex};

pub use export::{export_diagnostics, export_logs};
pub(crate) use files::{directory, ensure_directory};
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogPolicy {
    pub retention_days: u16,
    pub log_limit_mb: u16,
}

impl LogPolicy {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if ![0, 3, 7, 30].contains(&self.retention_days)
            || ![0, 100, 200, 500].contains(&self.log_limit_mb)
        {
            return Err("Unsupported Gateway log retention settings".into());
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    #[serde(flatten)]
    pub policy: LogPolicy,
    pub log_directory: String,
}

fn policy_path(config: &ConsoleConfig) -> PathBuf {
    config.state_dir.join("log-policy.json")
}

pub fn load_policy(config: &ConsoleConfig) -> Result<LogPolicy, String> {
    let path = policy_path(config);
    let bytes = match files::read_small(&path, 16 * 1024) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LogPolicy::default())
        }
        Err(_) => return Err("Cannot read Gateway log settings".into()),
    };
    let policy: LogPolicy =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid Gateway log settings")?;
    policy.validate()?;
    Ok(policy)
}

pub fn settings(config: &ConsoleConfig) -> Result<SettingsSnapshot, String> {
    Ok(SettingsSnapshot {
        policy: load_policy(config)?,
        log_directory: directory(config)?.display().to_string(),
    })
}

pub fn save_policy(config: &ConsoleConfig, policy: LogPolicy) -> Result<SettingsSnapshot, String> {
    policy.validate()?;
    let _guard = SETTINGS_LOCK
        .lock()
        .map_err(|_| "Gateway log settings lock poisoned")?;
    ensure_directory(&config.state_dir)?;
    let path = policy_path(config);
    files::reject_links(&path).map_err(|_| "Unsafe Gateway log settings path")?;
    let temporary = config
        .state_dir
        .join(format!(".log-policy-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| "Cannot stage Gateway log settings")?;
        file.write_all(
            &serde_json::to_vec_pretty(&policy).map_err(|_| "Cannot encode log settings")?,
        )
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot write Gateway log settings")?;
        fs::rename(&temporary, &path).map_err(|_| "Cannot publish Gateway log settings")?;
        settings(config)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
