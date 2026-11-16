//! Explicit user restore, separate from schema migration; never overwrite a live data root.
mod paths;
#[cfg(test)]
mod tests;
mod transaction;

pub use paths::{checked, validate_files, MAX_BYTES};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
};
pub(crate) use transaction::apply_pending;
pub const UNITS: [&str; 4] = ["local", "objects", "credentials", "gateway.json"];

/// Plaintext restore workspaces must not inherit a permissive Unix umask.
pub fn create_private_directory(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Files are already decrypted and validated by the backend. Staging is never active state.
pub fn stage(root: &Path, files: &BTreeMap<String, Vec<u8>>) -> io::Result<()> {
    validate_files(files)?;
    let _lock = operation_lock(root)?;
    let pending = checked(root, "restore-pending")?;
    if pending.exists() {
        return Err(io::Error::other(
            "A restore is already pending; restart Gateway first",
        ));
    }
    let staging = checked(root, "restore-staging")?;
    if staging.exists() {
        // The operation lock proves no staging writer remains after a process interruption.
        transaction::archive(root, &staging)?;
    }
    create_private_directory(&staging)?;
    let result = (|| {
        for (name, bytes) in files {
            let path = checked(&staging, name)?;
            fs::create_dir_all(
                path.parent()
                    .ok_or_else(|| io::Error::other("Invalid restore path"))?,
            )?;
            let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        write_marker(&staging.join("ready"), b"1")?;
        fs::rename(&staging, pending)
    })();
    if result.is_err() && staging.exists() {
        transaction::archive(root, &staging)?;
    }
    // Keep failed staging for diagnosis without preventing a later retry.
    result
}

pub fn cancel(root: &Path) -> io::Result<()> {
    let _lock = operation_lock(root)?;
    let pending = checked(root, "restore-pending")?;
    if !pending.exists() {
        return Ok(());
    }
    if checked(&pending, "started")?.exists() {
        return Err(io::Error::other("Restore publication has already started"));
    }
    transaction::archive(root, &pending)
}

pub fn pending(root: &Path) -> io::Result<bool> {
    Ok(checked(root, "restore-pending/ready")?.exists())
}

fn operation_lock(root: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(checked(root, "restore.lock")?)?;
    file.try_lock()
        .map_err(|_| io::Error::other("Another backup restore operation is active"))?;
    Ok(file)
}

fn write_marker(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
