use std::fs::{Metadata, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_DIRECTORY_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FILES: usize = 32;
const LOCK_NAME: &str = "writer.lock";

pub(super) fn write_debug_file(
    directory: &Path,
    name: &str,
    bytes: &[u8],
) -> io::Result<Option<PathBuf>> {
    if !valid_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid diagnostic filename",
        ));
    }
    if bytes.len() > MAX_FILE_BYTES {
        return Ok(None);
    }
    std::fs::create_dir_all(directory)?;
    let metadata = std::fs::symlink_metadata(directory)?;
    if !metadata.is_dir() || is_link(&metadata) {
        return Err(io::Error::other("diagnostic directory must not be a link"));
    }
    let directory = directory.canonicalize()?;
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    configure_private_open(&mut options);
    let lock = options.open(directory.join(LOCK_NAME))?;
    let metadata = lock.metadata()?;
    if !metadata.is_file() || is_link(&metadata) || metadata.len() != 0 {
        return Err(io::Error::other("invalid diagnostic lock file"));
    }
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Ok(None),
        Err(TryLockError::Error(error)) => return Err(error),
    }
    let (mut count, mut total, mut previous) = (0usize, 0u64, None);
    for (index, entry) in std::fs::read_dir(&directory)?.enumerate() {
        if index > MAX_FILES {
            return Ok(None);
        }
        let entry = entry?;
        if entry.file_name() == LOCK_NAME {
            continue;
        }
        let metadata = std::fs::symlink_metadata(entry.path())?;
        if !metadata.is_file() || is_link(&metadata) {
            return Err(io::Error::other(
                "diagnostic directory contains a nonregular entry",
            ));
        }
        count += 1;
        total = total.saturating_add(metadata.len());
        if entry.file_name() == name {
            previous = Some(metadata.len());
        }
    }
    if total > MAX_DIRECTORY_BYTES
        || (previous.is_none() && count >= MAX_FILES)
        || total - previous.unwrap_or(0) + bytes.len() as u64 > MAX_DIRECTORY_BYTES
    {
        return Ok(None);
    }
    let path = directory.join(name);
    // Publish only complete bytes. Preserve old snapshots on quota/lock/I/O failures.
    let pending_path = directory.join(format!(".pending-{}", uuid::Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    configure_private_open(&mut options);
    let mut file = options.open(&pending_path)?;
    let pending = PendingFile(pending_path);
    let result = file.write_all(bytes);
    drop(file);
    result?;
    std::fs::rename(&pending.0, &path)?;
    Ok(Some(path))
}

fn valid_name(name: &str) -> bool {
    name.len() <= 160
        && name.starts_with("gemini-canvas-")
        && !name.contains("..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && [".json", ".txt", ".jpg"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

struct PendingFile(PathBuf);

impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn configure_private_open(options: &mut OpenOptions) {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
}

fn is_link(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

#[cfg(test)]
#[path = "gemini_canvas_debug_file_tests.rs"]
mod tests;
