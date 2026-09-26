//! Publisher lock ownership, stale recovery and platform process probes.

use super::*;

struct PublishValidationLock {
    owner_token: String,
    released: bool,
}

impl PublishValidationLock {
    fn acquire() -> Result<Self, String> {
        let mut attempted_recovery = false;
        loop {
            match fs::create_dir(DIST_WEB_PUBLISH_LOCK_PATH) {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if !attempted_recovery && recover_stale_publish_lock()? {
                        attempted_recovery = true;
                        continue;
                    }
                    return Err(format!(
                        "web publish lock is active: {DIST_WEB_PUBLISH_LOCK_PATH}"
                    ));
                }
                Err(error) => {
                    return Err(format!(
                        "failed to acquire web publish lock {DIST_WEB_PUBLISH_LOCK_PATH}: {error}"
                    ));
                }
            }
        }

        let acquired_at = unix_time_nanos()?;
        let owner_token = format!("build-{}-{acquired_at}", std::process::id());
        let owner = serde_json::json!({
            "ownerToken": owner_token,
            "pid": std::process::id(),
            "acquiredAtUnixNanos": acquired_at.to_string(),
            "purpose": "web-ui-snapshot",
        });
        if let Err(error) = fs::write(DIST_WEB_PUBLISH_LOCK_OWNER_PATH, format!("{owner}\n")) {
            let _ = fs::remove_dir_all(DIST_WEB_PUBLISH_LOCK_PATH);
            return Err(format!(
                "failed to write web publish lock owner {DIST_WEB_PUBLISH_LOCK_OWNER_PATH}: {error}"
            ));
        }

        Ok(Self {
            owner_token,
            released: false,
        })
    }

    fn release(mut self) -> Result<(), String> {
        self.release_owned_lock()?;
        self.released = true;
        Ok(())
    }

    fn release_owned_lock(&self) -> Result<(), String> {
        let owner = fs::read_to_string(DIST_WEB_PUBLISH_LOCK_OWNER_PATH).map_err(|error| {
            format!(
                "failed to read web publish lock owner {DIST_WEB_PUBLISH_LOCK_OWNER_PATH}: {error}"
            )
        })?;
        let owner: serde_json::Value = serde_json::from_str(&owner).map_err(|error| {
            format!(
                "failed to parse web publish lock owner {DIST_WEB_PUBLISH_LOCK_OWNER_PATH}: {error}"
            )
        })?;
        let current_owner = owner
            .get("ownerToken")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if current_owner != self.owner_token {
            return Err(format!(
                "web publish lock owner changed before release: {DIST_WEB_PUBLISH_LOCK_PATH}"
            ));
        }
        fs::remove_dir_all(DIST_WEB_PUBLISH_LOCK_PATH).map_err(|error| {
            format!("failed to release web publish lock {DIST_WEB_PUBLISH_LOCK_PATH}: {error}")
        })
    }
}

impl Drop for PublishValidationLock {
    fn drop(&mut self) {
        if !self.released {
            let _ = self.release_owned_lock();
        }
    }
}

pub(super) fn unix_time_nanos() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))
}

fn recover_stale_publish_lock() -> Result<bool, String> {
    recover_stale_publish_lock_with(process_is_alive)
}

pub(super) fn recover_stale_publish_lock_with<F>(is_process_alive: F) -> Result<bool, String>
where
    F: FnOnce(u32) -> Result<bool, String>,
{
    // Never infer staleness from timestamps. Recovery requires an intact owner token,
    // a parseable PID, and a conclusive OS-level answer that the process exited.
    let owner_text = match fs::read_to_string(DIST_WEB_PUBLISH_LOCK_OWNER_PATH) {
        Ok(owner_text) => owner_text,
        // The one exception is a lock directory with no owner file at all, left by a
        // publisher killed between creating the directory and writing the file.
        // Nothing can ever prove that owner gone, so without this every later build
        // fails until someone deletes the directory by hand.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return recover_ownerless_publish_lock();
        }
        Err(_) => return Ok(false),
    };
    let owner: serde_json::Value = match serde_json::from_str(&owner_text) {
        Ok(owner) => owner,
        Err(_) => return Ok(false),
    };
    let owner_token = match owner
        .get("ownerToken")
        .and_then(serde_json::Value::as_str)
        .filter(|owner_token| !owner_token.trim().is_empty())
    {
        Some(owner_token) => owner_token,
        None => return Ok(false),
    };
    let owner_pid = match owner
        .get("pid")
        .and_then(serde_json::Value::as_u64)
        .and_then(|pid| u32::try_from(pid).ok())
        .filter(|pid| *pid != 0)
    {
        Some(owner_pid) => owner_pid,
        None => return Ok(false),
    };

    let owner_is_alive = match is_process_alive(owner_pid) {
        Ok(owner_is_alive) => owner_is_alive,
        Err(_) => return Ok(false),
    };
    if owner_is_alive {
        return Ok(false);
    }

    let owner_after_probe = match fs::read_to_string(DIST_WEB_PUBLISH_LOCK_OWNER_PATH) {
        Ok(owner_after_probe) => owner_after_probe,
        Err(_) => return Ok(false),
    };
    if owner_after_probe != owner_text {
        return Ok(false);
    }

    // Rename first so cleanup can never remove a new publisher lock created at the
    // canonical path after this stale owner is detached.
    let quarantine_path = PathBuf::from(format!(
        "{DIST_WEB_PUBLISH_LOCK_PATH}.stale-{}-{}",
        std::process::id(),
        unix_time_nanos()?
    ));
    match fs::rename(DIST_WEB_PUBLISH_LOCK_PATH, &quarantine_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Ok(false),
    }

    let quarantined_owner = fs::read_to_string(quarantine_path.join("owner.json")).map_err(
        |error| {
            format!(
                "reclaimed stale web publish lock for owner {owner_token} (pid {owner_pid}), but failed to verify its quarantined owner: {error}"
            )
        },
    )?;
    if quarantined_owner != owner_text {
        return Err(format!(
            "web publish lock owner changed while reclaiming stale owner {owner_token} (pid {owner_pid})"
        ));
    }
    fs::remove_dir_all(&quarantine_path).map_err(|error| {
        format!(
            "reclaimed stale web publish lock for owner {owner_token} (pid {owner_pid}), but failed to remove {}: {error}",
            quarantine_path.display()
        )
    })?;
    println!(
        "cargo:warning=reclaimed a stale web publish lock after proving its owner process no longer exists"
    );
    Ok(true)
}

/// Reclaims a lock directory that never received an owner file.
///
/// The guards keep a live publisher safe without reading an owner: the directory has
/// to still be empty, it has to have sat untouched for longer than the gap between
/// creating it and writing the owner file could ever be, and the removal is
/// non-recursive so it fails the moment a racing publisher has written anything.
fn recover_ownerless_publish_lock() -> Result<bool, String> {
    let metadata = match fs::symlink_metadata(DIST_WEB_PUBLISH_LOCK_PATH) {
        Ok(metadata) => metadata,
        // Someone already cleared it, so the caller only has to retry the create.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(_) => return Ok(false),
    };
    if !metadata.is_dir() {
        return Ok(false);
    }
    // Bindings made in a pattern guard are immutable, so the emptiness check has to
    // happen in the arm body where `entries` can be advanced.
    match fs::read_dir(DIST_WEB_PUBLISH_LOCK_PATH) {
        Ok(mut entries) => {
            // A directory holding files is not the half-created case this handles.
            if entries.next().is_some() {
                return Ok(false);
            }
        }
        Err(_) => return Ok(false),
    }
    let idle_for = match metadata
        .modified()
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
    {
        Some(idle_for) => idle_for,
        None => return Ok(false),
    };
    if idle_for < PUBLISH_LOCK_OWNERLESS_GRACE {
        return Ok(false);
    }
    match fs::remove_dir(DIST_WEB_PUBLISH_LOCK_PATH) {
        // Only an empty directory is removable this way, so a racing publisher that
        // refilled it surfaces here as an error and keeps its lock.
        Ok(()) => {
            println!(
                "cargo:warning=reclaimed a web publish lock directory that was left without an owner file"
            );
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(_) => Ok(false),
    }
}

#[cfg(windows)]
pub(super) fn process_is_alive(pid: u32) -> Result<bool, String> {
    use std::ffi::c_void;

    type Handle = *mut c_void;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    const ERROR_ACCESS_DENIED: u32 = 5;
    const ERROR_INVALID_PARAMETER: u32 = 87;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> Handle;
        fn GetExitCodeProcess(process: Handle, exit_code: *mut u32) -> i32;
        fn CloseHandle(object: Handle) -> i32;
        fn GetLastError() -> u32;
    }

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        let error = unsafe { GetLastError() };
        return match error {
            ERROR_INVALID_PARAMETER => Ok(false),
            ERROR_ACCESS_DENIED => Ok(true),
            _ => Err(format!(
                "failed to inspect web publish lock owner process {pid}: Windows error {error}"
            )),
        };
    }

    let mut exit_code = 0_u32;
    let query_result = unsafe { GetExitCodeProcess(process, &mut exit_code) };
    let query_error = if query_result == 0 {
        Some(unsafe { GetLastError() })
    } else {
        None
    };
    let _ = unsafe { CloseHandle(process) };
    if let Some(error) = query_error {
        return Err(format!(
            "failed to query web publish lock owner process {pid}: Windows error {error}"
        ));
    }
    Ok(exit_code == STILL_ACTIVE)
}

#[cfg(unix)]
pub(super) fn process_is_alive(pid: u32) -> Result<bool, String> {
    const EPERM: i32 = 1;
    const ESRCH: i32 = 3;

    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }

    let Ok(pid) = i32::try_from(pid) else {
        return Ok(false);
    };
    if unsafe { kill(pid, 0) } == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(ESRCH) => Ok(false),
        Some(EPERM) => Ok(true),
        _ => Err(format!(
            "failed to inspect web publish lock owner process {pid}: {error}"
        )),
    }
}

#[cfg(not(any(unix, windows)))]
pub(super) fn process_is_alive(pid: u32) -> Result<bool, String> {
    Err(format!(
        "cannot prove whether web publish lock owner process {pid} exists on this platform"
    ))
}

pub(super) fn with_publish_validation_lock<T>(
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let validation_lock = PublishValidationLock::acquire()?;
    let operation_result = operation();
    let release_result = validation_lock.release();
    match (operation_result, release_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(operation_error), Err(release_error)) => Err(format!(
            "{operation_error}; additionally failed to release validation lock: {release_error}"
        )),
    }
}
