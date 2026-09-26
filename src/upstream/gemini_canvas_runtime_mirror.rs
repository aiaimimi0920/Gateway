//! Local sidecar fallback with validated keys and bounded regular-file reads.
use crate::object_storage::local_object_path;
use crate::upstream::gemini_canvas_runtime_paths::gemini_canvas_local_runtime_root;
use serde_json::Value;
use std::fs::{Metadata, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const MAX_MIRROR_BYTES: usize = 32 * 1024 * 1024;

pub(crate) fn gemini_canvas_runtime_mirror_json_path(key: &str) -> Option<PathBuf> {
    mirror_path(key, &mirror_roots())
}

pub(crate) fn read_gemini_canvas_runtime_mirror_json(key: &str) -> Option<Value> {
    read_mirror(key, &mirror_roots())
}

fn mirror_roots() -> [PathBuf; 2] {
    [
        gemini_canvas_local_runtime_root().join("ai-gateway-objects"),
        PathBuf::from("..")
            .join(".runtime")
            .join("ai-gateway-objects"),
    ]
}

fn mirror_path(key: &str, roots: &[PathBuf; 2]) -> Option<PathBuf> {
    let key = key.trim();
    if key.starts_with(['/', '\\']) {
        return None;
    }
    // Keep existing relative Windows keys, but never strip an absolute prefix.
    let key = key
        .trim_start_matches("./")
        .trim_start_matches(".\\")
        .replace('\\', "/");
    let mut missing = None;
    for root in roots {
        let path = checked_path(root, &key)?;
        if path.exists() {
            return Some(path);
        }
        missing.get_or_insert(path);
    }
    missing
}

fn checked_path(root: &Path, key: &str) -> Option<PathBuf> {
    let path = local_object_path(root, key).ok()?;
    let mut current = root.to_path_buf();
    for segment in std::iter::once(None).chain(key.split('/').map(Some)) {
        if let Some(segment) = segment {
            current.push(segment);
        }
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if is_link(&metadata) || (current != path && !metadata.is_dir()) {
                    return None;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Some(path),
            Err(_) => return None,
        }
    }
    Some(path)
}

fn read_mirror(key: &str, roots: &[PathBuf; 2]) -> Option<Value> {
    let path = mirror_path(key, roots)?;
    let mut options = OpenOptions::new();
    options.read(true);
    // Root/parent checks assume owner-controlled directories. These flags also
    // reject a redirected final component and avoid blocking on Unix FIFOs.
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
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || is_link(&metadata) {
        return None;
    }
    read_json(file, metadata.len(), MAX_MIRROR_BYTES)
}

fn read_json(mut reader: impl Read, known_length: u64, limit: usize) -> Option<Value> {
    if known_length > limit as u64 {
        return None;
    }
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 16 * 1024];
    loop {
        let read = match reader.read(&mut chunk) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result.ok()?,
        };
        if read == 0 {
            break;
        }
        // Metadata may be stale. Check actual bytes before growing the buffer.
        if bytes.len().checked_add(read)? > limit {
            return None;
        }
        bytes.try_reserve(read).ok()?;
        bytes.extend_from_slice(&chunk[..read]);
    }
    let text = String::from_utf8(bytes).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
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
#[path = "gemini_canvas_runtime_mirror_tests.rs"]
mod tests;
