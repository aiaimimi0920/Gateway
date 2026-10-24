//! Opt-in backend startup cleanup. Only recognized inactive logs are eligible.
use super::{files, LogPolicy};
use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime},
};

fn historical_lease(path: &Path) -> Result<Option<fs::File>, String> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Deny write sharing: an active writer makes this fail, even in another process.
        options.share_mode(5).custom_flags(0x00200000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if matches!(error.raw_os_error(), Some(32 | 33)) => return Ok(None),
        Err(_) => return Err("Cannot lease historical Gateway log".into()),
    };
    let metadata = file
        .metadata()
        .map_err(|_| "Cannot inspect historical Gateway log")?;
    if !metadata.is_file() || files::reparse(&metadata) {
        return Ok(None);
    }
    if files::validate_handle(&file).is_err() {
        return Ok(None);
    }
    #[cfg(unix)]
    if file.try_lock().is_err() {
        return Ok(None);
    }
    Ok(Some(file))
}

pub(super) fn clean(directory: &Path, current: &Path, policy: &LogPolicy) -> Result<(), String> {
    if policy.retention_days == 0 && policy.log_limit_mb == 0 {
        return Ok(());
    }
    files::reject_links(directory).map_err(|_| "Unsafe Gateway log directory")?;
    let mut logs = Vec::new();
    for (index, entry) in fs::read_dir(directory)
        .map_err(|_| "Cannot list Gateway logs")?
        .enumerate()
    {
        if index >= files::MAX_ENTRIES {
            return Err("Too many Gateway log entries".into());
        }
        let entry = entry.map_err(|_| "Cannot inspect Gateway log")?;
        let path = entry.path();
        if path == current || !files::owned_name(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let metadata = fs::symlink_metadata(&path).map_err(|_| "Cannot inspect Gateway log")?;
        if !metadata.is_file() || files::reparse(&metadata) {
            continue;
        }
        let Some(lease) = historical_lease(&path)? else {
            continue;
        };
        let metadata = lease
            .metadata()
            .map_err(|_| "Cannot inspect Gateway log lease")?;
        logs.push((
            metadata
                .modified()
                .map_err(|_| "Cannot inspect log timestamp")?,
            path,
            metadata.len(),
            lease,
        ));
    }
    logs.sort_by_key(|entry| entry.0);
    let mut total: u64 = logs.iter().map(|entry| entry.2).sum();
    let budget = u64::from(policy.log_limit_mb) * 1024 * 1024;
    for (modified, path, length, _lease) in logs {
        let expired = policy.retention_days > 0
            && SystemTime::now()
                .duration_since(modified)
                .unwrap_or_default()
                > Duration::from_secs(u64::from(policy.retention_days) * 86400);
        if expired || (budget > 0 && total > budget) {
            files::reject_links(&path).map_err(|_| "Unsafe historical Gateway log")?;
            if let Err(error) = fs::remove_file(&path) {
                if matches!(error.raw_os_error(), Some(32 | 33)) {
                    continue;
                }
                return Err("Cannot clean historical Gateway logs".into());
            }
            total = total.saturating_sub(length);
        }
    }
    Ok(())
}
