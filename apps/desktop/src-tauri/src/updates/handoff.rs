//! A per-attempt, PID-bound acknowledgement is emitted only after the relevant startup succeeds.
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use tauri::AppHandle;

fn receipt(root: &Path) -> Result<Option<PathBuf>, String> {
    let Some(receipt) = std::env::var_os("GATEWAY_UPDATE_RECEIPT") else {
        return Ok(None);
    };
    let receipt = PathBuf::from(receipt);
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let base = exe
        .parent()
        .and_then(Path::parent)
        .ok_or("Invalid update directory")?;
    let base = base.canonicalize().map_err(|e| e.to_string())?;
    let updates = root
        .join("updates")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let name = receipt
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid update receipt")?;
    if !base.starts_with(updates)
        || receipt.parent().and_then(|p| p.canonicalize().ok()) != Some(base)
        || !name.strip_prefix("started-").is_some_and(|nonce| {
            !nonce.is_empty() && nonce.len() < 50 && nonce.bytes().all(|b| b.is_ascii_digit())
        })
    {
        return Err("Invalid update receipt location".into());
    }
    Ok(Some(receipt))
}
pub fn acknowledge(root: &Path) -> Result<(), String> {
    let Some(receipt) = receipt(root)? else {
        return Ok(());
    };
    let bytes = serde_json::to_vec(&serde_json::json!({"pid":std::process::id(),"ready":true}))
        .map_err(|e| e.to_string())?;
    let temporary = receipt.with_extension("tmp");
    fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    fs::rename(temporary, receipt).map_err(|e| e.to_string())
}
pub fn watch(app: &AppHandle, root: &Path) -> Result<(), String> {
    let Some(receipt) = receipt(root)? else {
        return Ok(());
    };
    let app = app.clone();
    std::thread::spawn(move || {
        for _ in 0..180 {
            if receipt.with_extension("cancel").exists() {
                app.exit(0);
                return;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        if !receipt.exists() {
            app.exit(0);
        }
    });
    Ok(())
}
