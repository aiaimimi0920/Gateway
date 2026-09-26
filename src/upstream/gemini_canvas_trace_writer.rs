use crate::error::sanitize_provider_error_message;
use std::fs::{Metadata, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_TRACE_BYTES: u64 = 1024 * 1024;

pub(crate) fn append_bounded_gemini_canvas_trace(
    path: &Path,
    stage: &str,
    detail: &str,
) -> io::Result<bool> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    append_trace_entry_at(path, stage, detail, millis, MAX_TRACE_BYTES)
}

fn append_trace_entry_at(
    path: &Path,
    stage: &str,
    detail: &str,
    millis: u128,
    max_bytes: u64,
) -> io::Result<bool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).read(true).append(true);
    configure_trace_open(&mut options);
    let mut file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || is_reparse_point(&metadata) {
        return Err(io::Error::other("trace target must be a regular file"));
    }
    // Optional diagnostics must not queue behind another process or replace old evidence.
    match file.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Ok(false),
        Err(TryLockError::Error(error)) => return Err(error),
    }
    let length = file.metadata()?.len();
    if length >= max_bytes {
        return Ok(false);
    }
    let stage: String = sanitize_provider_error_message(stage)
        .chars()
        .take(64)
        .collect();
    let detail = sanitize_provider_error_message(detail);
    let entry = format!("{millis}\t{stage}\t{detail}\n");
    if entry.len() as u64 > max_bytes - length {
        return Ok(false);
    }
    file.write_all(entry.as_bytes())?;
    Ok(true)
}

fn configure_trace_open(options: &mut OpenOptions) {
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
        options.custom_flags(libc::O_NOFOLLOW);
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &Metadata) -> bool {
    false
}

#[cfg(test)]
#[path = "gemini_canvas_trace_writer_tests.rs"]
mod tests;
