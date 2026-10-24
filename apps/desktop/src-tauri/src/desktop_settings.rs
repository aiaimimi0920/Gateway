//! Trusted-window preferences, separate from backend credentials and connection drafts.
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::{path::Path, sync::Mutex};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Preferences {
    pub close_behavior: CloseBehavior,
    // Legacy values remain readable for compatibility; backend log-policy.json now owns retention.
    pub retention_days: u16,
    pub log_limit_mb: u16,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloseBehavior {
    Tray,
    #[default]
    Exit,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    #[serde(flatten)]
    pub preferences: Preferences,
    pub autostart: bool,
    pub autostart_supported: bool,
}
#[derive(Default)]
pub struct SettingsState(pub Mutex<Preferences>);

pub fn load(path: &Path) -> Result<Preferences, String> {
    let settings = match std::fs::File::open(path) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take(16 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Cannot read desktop settings")?;
            if bytes.len() > 16 * 1024 {
                return Err("Desktop settings file is too large".into());
            }
            serde_json::from_slice(&bytes).map_err(|_| "Invalid desktop settings")?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
        Err(_) => return Err("Cannot read desktop settings".into()),
    };
    validate(&settings)?;
    Ok(settings)
}
fn validate(settings: &Preferences) -> Result<(), String> {
    if ![0, 3, 7, 30].contains(&settings.retention_days)
        || ![0, 100, 200, 500].contains(&settings.log_limit_mb)
    {
        return Err("Unsupported log retention settings".into());
    }
    Ok(())
}
fn save(path: &Path, settings: &Preferences) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    let pending = path.with_extension(format!("json.pending-{}", std::process::id()));
    std::fs::write(&pending, bytes).map_err(|_| "Cannot write desktop settings")?;
    std::fs::rename(pending, path).map_err(|_| "Cannot publish desktop settings".into())
}
#[tauri::command]
pub fn load_gateway_desktop_preferences(app: AppHandle) -> Result<Snapshot, String> {
    let state = app.state::<SettingsState>();
    let settings = state
        .0
        .lock()
        .map_err(|_| "Settings lock poisoned")?
        .clone();
    Ok(Snapshot {
        preferences: settings,
        autostart: crate::autostart::enabled()?,
        autostart_supported: cfg!(windows),
    })
}
#[tauri::command]
pub fn save_gateway_desktop_preferences(
    app: AppHandle,
    settings: Preferences,
    autostart: bool,
) -> Result<Snapshot, String> {
    validate(&settings)?;
    let state = app.state::<SettingsState>();
    let mut guard = state.0.lock().map_err(|_| "Settings lock poisoned")?;
    if settings.close_behavior == CloseBehavior::Tray && app.tray_by_id("gateway-tray").is_none() {
        return Err("Gateway tray is unavailable; close behavior was not changed".into());
    }
    let previous = crate::autostart::enabled()?;
    let path = crate::paths::gateway_app_dir()?.join("desktop-settings.json");
    crate::autostart::set(autostart)?;
    if let Err(error) = save(&path, &settings) {
        crate::autostart::set(previous)
            .map_err(|_| "Settings save and autostart rollback failed; check startup settings")?;
        return Err(error);
    }
    *guard = settings.clone();
    Ok(Snapshot {
        preferences: settings,
        autostart,
        autostart_supported: cfg!(windows),
    })
}
pub fn minimize_to_tray(app: &AppHandle) -> bool {
    let state = app.state::<SettingsState>();
    state
        .0
        .lock()
        .map(|settings| settings.close_behavior == CloseBehavior::Tray)
        .unwrap_or(false)
        && app.tray_by_id("gateway-tray").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_preserve_exit_and_existing_logs() {
        let value: Preferences = serde_json::from_str("{}").unwrap();
        assert_eq!(value, Preferences::default());
        assert_eq!(value.close_behavior, CloseBehavior::Exit);
        assert_eq!(value.retention_days, 0);
        assert_eq!(value.log_limit_mb, 0);
    }
    #[test]
    fn rejects_unsupported_values_and_secrets() {
        assert!(validate(&Preferences {
            retention_days: 999,
            ..Default::default()
        })
        .is_err());
        assert!(validate(&Preferences {
            log_limit_mb: 1,
            ..Default::default()
        })
        .is_err());
        assert!(serde_json::from_str::<Preferences>(r#"{"token":"secret"}"#).is_err());
    }

    #[test]
    fn atomic_save_round_trips_and_corruption_is_not_overwritten() {
        let dir = std::env::temp_dir().join(format!(
            "gateway-preferences-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("desktop-settings.json");
        assert_eq!(load(&path).unwrap(), Preferences::default());
        let settings = Preferences {
            close_behavior: CloseBehavior::Tray,
            retention_days: 7,
            log_limit_mb: 200,
        };
        save(&path, &settings).unwrap();
        save(&path, &settings).unwrap();
        assert_eq!(load(&path).unwrap(), settings);
        std::fs::write(&path, b"broken").unwrap();
        assert!(load(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
        std::fs::write(&path, vec![b' '; 16 * 1024 + 1]).unwrap();
        assert!(load(&path).unwrap_err().contains("too large"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
