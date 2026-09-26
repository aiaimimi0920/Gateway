//! Shared static containment boundary for folder-sync reads, writes and deletion.
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crate::error::GatewayError;
use dashmap::DashMap;

pub(super) fn root_operation_lock(root: &Path) -> Arc<Mutex<()>> {
    static ROOT_LOCKS: OnceLock<DashMap<PathBuf, Arc<Mutex<()>>>> = OnceLock::new();
    ROOT_LOCKS
        .get_or_init(DashMap::new)
        .entry(root.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

pub(super) fn validated_relative_path(value: &str) -> Result<Option<PathBuf>, GatewayError> {
    // DB source_path stores 512 Unicode characters; truncating a path changes its identity.
    if value
        .chars()
        .take(super::limits::SOURCE_PATH_CHARS + 1)
        .count()
        > super::limits::SOURCE_PATH_CHARS
    {
        return Err(invalid_path());
    }
    if value.trim().is_empty() {
        return Ok(None);
    }
    if value.starts_with(['/', '\\']) {
        return Err(invalid_path());
    }
    let mut relative = PathBuf::new();
    for segment in value
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty())
    {
        if segment != segment.trim()
            || segment.ends_with('.')
            || segment
                .chars()
                .any(|ch| ch.is_control() || ":<>\"|?*".contains(ch))
            || Path::new(segment)
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
            || is_device_name(segment)
        {
            return Err(invalid_path());
        }
        relative.push(segment);
    }
    Ok(Some(relative))
}

fn is_device_name(segment: &str) -> bool {
    let stem = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .trim_end_matches(' ');
    let upper = stem.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || upper
        .strip_prefix("COM")
        .or_else(|| upper.strip_prefix("LPT"))
        .is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2"
                    | "3"
                    | "4"
                    | "5"
                    | "6"
                    | "7"
                    | "8"
                    | "9"
                    | "\u{b9}"
                    | "\u{b2}"
                    | "\u{b3}"
            )
        })
}

pub(super) fn invalid_path() -> GatewayError {
    GatewayError::bad_request("invalid provider credential folder sync relative path")
        .with_code("provider_credential_folder_sync_path_invalid")
}

fn path_escape() -> GatewayError {
    GatewayError::bad_request("provider credential folder sync rejects linked or escaping paths")
        .with_code("provider_credential_folder_sync_path_escape")
}

pub(super) fn relative_key(relative: &Path) -> Result<String, GatewayError> {
    let mut segments = Vec::new();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(invalid_path());
        };
        let segment = segment.to_str().ok_or_else(invalid_path)?;
        if segment.contains(['/', '\\']) {
            return Err(invalid_path());
        }
        segments.push(segment);
    }
    let key = segments.join("/");
    validated_relative_path(&key)?.ok_or_else(invalid_path)?;
    Ok(key)
}

pub(super) struct SyncRoot {
    root: PathBuf,
}

impl SyncRoot {
    pub(super) fn new(root: &Path) -> Result<Self, GatewayError> {
        let root = fs::canonicalize(root).map_err(|error| inspect_error(root, error))?;
        Ok(Self { root })
    }

    pub(super) fn path(&self) -> &Path {
        &self.root
    }

    pub(super) fn resolve(&self, relative: &Path) -> Result<PathBuf, GatewayError> {
        // The configured root is trusted, including a root link. Descendants are not.
        // Static checks reject linked descendants, but concurrent replacement remains a
        // separate handle-relative access concern.
        relative_key(relative)?;
        let mut absolute = self.root.clone();
        let mut existing = self.root.clone();
        for component in relative.components() {
            absolute.push(component);
            let metadata = match fs::symlink_metadata(&absolute) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == ErrorKind::NotFound => break,
                Err(error) => return Err(inspect_error(&absolute, error)),
            };
            if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
                return Err(path_escape());
            }
            if metadata.is_file() && is_hard_link_alias(&absolute, &metadata)? {
                return Err(path_escape());
            }
            if !metadata.is_file() && !metadata.is_dir() {
                return Err(invalid_path());
            }
            existing.clone_from(&absolute);
        }
        let resolved =
            fs::canonicalize(&existing).map_err(|error| inspect_error(&existing, error))?;
        if !resolved.starts_with(&self.root) {
            return Err(path_escape());
        }
        Ok(self.root.join(relative))
    }
}

pub(super) fn delete_file(root: &Path, relative: &Path) -> Result<bool, GatewayError> {
    let operation_lock = root_operation_lock(root);
    let _operation_guard = operation_lock
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = match fs::canonicalize(root) {
        Ok(root) => SyncRoot { root },
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(file_error(root, error)),
    };
    let absolute = root.resolve(relative)?;
    // Delete the checked pathname, never a canonicalized link target.
    match fs::remove_file(&absolute) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(file_error(&absolute, error)),
    }
}

fn inspect_error(path: &Path, error: std::io::Error) -> GatewayError {
    GatewayError::server_error(format!(
        "inspect provider credential path {}: {error}",
        path.display()
    ))
}

fn file_error(path: &Path, error: std::io::Error) -> GatewayError {
    GatewayError::server_error(format!(
        "delete provider credential file {}: {error}",
        path.display()
    ))
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn is_hard_link_alias(_path: &Path, metadata: &fs::Metadata) -> Result<bool, GatewayError> {
    use std::os::unix::fs::MetadataExt;
    Ok(metadata.nlink() > 1)
}

#[cfg(windows)]
fn is_hard_link_alias(path: &Path, _metadata: &fs::Metadata) -> Result<bool, GatewayError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let file = fs::File::open(path).map_err(|error| inspect_error(path, error))?;
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    let success = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) };
    if success == 0 {
        return Err(inspect_error(path, std::io::Error::last_os_error()));
    }
    Ok(information.nNumberOfLinks > 1)
}

#[cfg(not(any(unix, windows)))]
fn is_hard_link_alias(_path: &Path, _metadata: &fs::Metadata) -> Result<bool, GatewayError> {
    Ok(false)
}
