//! Fixed 256 lock stripes: no caller-controlled path and no unlink; collisions conservatively report busy.
use super::{unavailable, GatewayError, LocalRuntime};
use std::fs::{File, OpenOptions, TryLockError};
pub(super) async fn acquire(runtime: &LocalRuntime, binding: &str) -> Result<File, GatewayError> {
    let path = runtime
        .lock_directory
        .join(format!("freebuff-session-{}.lock", &binding[..2]));
    tokio::task::spawn_blocking(move || {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|_| unavailable())?;
        match file.try_lock() {
            Ok(()) => Ok(file),
            Err(TryLockError::WouldBlock) => Err(GatewayError::conflict(
                "FreeBuff session claim is busy; no second admission was sent.",
            )
            .with_code("freebuff_session_claim_busy")),
            Err(TryLockError::Error(_)) => Err(unavailable()),
        }
    })
    .await
    .map_err(|_| unavailable())?
}
