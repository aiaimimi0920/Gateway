//! Small, explicit diagnostic reports and bounded, sanitized log downloads.
use super::{files, redaction, settings};
use crate::config::Config;
use serde::Serialize;
use std::sync::Mutex;

static EXPORT_LOCK: Mutex<()> = Mutex::new(());
const MAX_FILES: usize = 16;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    pub file_name: String,
    pub mime_type: &'static str,
    pub content: String,
    pub truncated: bool,
}

pub fn export_diagnostics(config: &Config) -> Result<Download, String> {
    // Never serialize Config: it contains credentials, environment URLs and storage secrets.
    let report = serde_json::json!({
        "format": "gateway-backend-diagnostics-v1",
        "createdAt": time::OffsetDateTime::now_utc().unix_timestamp(),
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "processId": std::process::id(),
        "runtimeRole": format!("{:?}", config.runtime_role).to_lowercase(),
        "storageMode": format!("{:?}", config.storage_mode).to_lowercase(),
        "managementUiEnabled": config.console.management_ui_enabled,
        "logs": settings(&config.console)?,
    });
    Ok(Download {
        file_name: "gateway-diagnostics.json".into(),
        mime_type: "application/json;charset=utf-8",
        content: serde_json::to_string_pretty(&report)
            .map_err(|_| "Cannot encode Gateway diagnostics")?,
        truncated: false,
    })
}

pub fn export_logs(config: &super::super::ConsoleConfig) -> Result<Download, String> {
    let _guard = EXPORT_LOCK
        .try_lock()
        .map_err(|_| "Another Gateway log export is in progress")?;
    let paths = files::logs(config)?;
    let mut truncated = paths.len() > MAX_FILES;
    let mut content = String::from("Gateway backend logs (sanitized; newest files first)\n");
    for (index, path) in paths.iter().take(MAX_FILES).enumerate() {
        let (text, partial) =
            files::tail(path).map_err(|_| "Cannot read Gateway log for export")?;
        truncated |= partial;
        // Use an ordinal, not a filesystem name which could itself contain a credential.
        content.push_str(&format!(
            "\n--- Log {}{} ---\n",
            index + 1,
            if partial { " (tail only)" } else { "" }
        ));
        content.push_str(&redaction::sanitize(&text));
        content.push('\n');
    }
    if truncated {
        content.push_str("\n[TRUNCATED: at most 16 files, 256 KiB per file]\n");
    }
    Ok(Download {
        file_name: "gateway-logs.log".into(),
        mime_type: "text/plain;charset=utf-8",
        content,
        truncated,
    })
}
