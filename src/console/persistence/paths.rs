//! Filesystem path admission and bootstrap preserve link rejection and lock sharing.

use super::PersistenceError;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Component, Path, PathBuf};

pub(super) fn absolute_lexical(path: &Path) -> Result<PathBuf, PersistenceError> {
    let absolute = std::path::absolute(path)
        .map_err(|error| PersistenceError::io("resolving an absolute path", error))?;
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(PersistenceError::path_escape(
                        "Mutable console path escapes its filesystem root",
                    ));
                }
            }
        }
    }
    Ok(normalized)
}

#[cfg(windows)]
pub(super) fn validate_platform_path(path: &Path) -> Result<(), PersistenceError> {
    use std::path::Prefix;

    let mut saw_prefix = false;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                saw_prefix = true;
                match prefix.kind() {
                    Prefix::Disk(_)
                    | Prefix::UNC(_, _)
                    | Prefix::VerbatimDisk(_)
                    | Prefix::VerbatimUNC(_, _) => {}
                    Prefix::DeviceNS(_) | Prefix::Verbatim(_) => {
                        return Err(PersistenceError::new(
                            "console_unsupported_platform_operation",
                            "Windows device namespace paths are not supported for console persistence",
                        ));
                    }
                }
            }
            Component::Normal(value) => validate_windows_normal_component(value)?,
            Component::ParentDir => {
                return Err(PersistenceError::path_escape(
                    "Mutable console path contains parent traversal",
                ));
            }
            Component::RootDir | Component::CurDir => {}
        }
    }
    if !saw_prefix || !path.is_absolute() {
        return Err(PersistenceError::new(
            "console_unsupported_platform_operation",
            "Windows console persistence requires an absolute disk or UNC path",
        ));
    }
    Ok(())
}

#[cfg(windows)]
pub(super) fn validate_unresolved_platform_path(path: &Path) -> Result<(), PersistenceError> {
    use std::path::Prefix;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => match prefix.kind() {
                Prefix::Disk(_)
                | Prefix::UNC(_, _)
                | Prefix::VerbatimDisk(_)
                | Prefix::VerbatimUNC(_, _) => {}
                Prefix::DeviceNS(_) | Prefix::Verbatim(_) => {
                    return Err(PersistenceError::new(
                        "console_unsupported_platform_operation",
                        "Windows device namespace paths are not supported for console persistence",
                    ));
                }
            },
            Component::Normal(value) => validate_windows_normal_component(value)?,
            Component::RootDir | Component::CurDir | Component::ParentDir => {}
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn validate_unresolved_platform_path(_path: &Path) -> Result<(), PersistenceError> {
    Ok(())
}

#[cfg(windows)]
fn validate_windows_normal_component(value: &std::ffi::OsStr) -> Result<(), PersistenceError> {
    use std::os::windows::ffi::OsStrExt;

    if value.encode_wide().any(|unit| unit == b':' as u16) {
        return Err(PersistenceError::new(
            "console_unsupported_platform_operation",
            "NTFS alternate data stream paths are not supported for console persistence",
        ));
    }
    let value = value.to_string_lossy();
    if value.ends_with(['.', ' ']) || is_windows_reserved_name(&value) {
        return Err(PersistenceError::new(
            "console_unsupported_platform_operation",
            "Windows reserved names and trailing dot/space components are not supported for console persistence",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn is_windows_reserved_name(value: &str) -> bool {
    let basename = value.split('.').next().unwrap_or(value);
    let uppercase = basename.to_ascii_uppercase();
    matches!(uppercase.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || uppercase.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || uppercase.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
}

#[cfg(not(windows))]
pub(super) fn validate_platform_path(path: &Path) -> Result<(), PersistenceError> {
    if !path.is_absolute() {
        return Err(PersistenceError::new(
            "console_unsupported_platform_operation",
            "Console persistence requires an absolute path",
        ));
    }
    Ok(())
}

pub(super) fn reject_existing_links(path: &Path) -> Result<(), PersistenceError> {
    let mut current = PathBuf::new();
    let mut missing = false;
    for component in path.components() {
        current.push(component.as_os_str());
        if missing || matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => reject_link_metadata(&current, &metadata)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => missing = true,
            Err(error) => return Err(PersistenceError::io("checking path components", error)),
        }
    }
    Ok(())
}

pub(super) fn ensure_secure_directory(path: &Path) -> Result<(), PersistenceError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                reject_link_metadata(&current, &metadata)?;
                if !metadata.is_dir() {
                    return Err(PersistenceError::corruption(format!(
                        "Console state component '{}' is not a directory",
                        current.display()
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => {
                        return Err(PersistenceError::io(
                            "creating console state directory",
                            error,
                        ));
                    }
                }
                let metadata = fs::symlink_metadata(&current)
                    .map_err(|error| PersistenceError::io("checking created directory", error))?;
                reject_link_metadata(&current, &metadata)?;
                if !metadata.is_dir() {
                    return Err(PersistenceError::corruption(
                        "Created console state component is not a directory",
                    ));
                }
            }
            Err(error) => return Err(PersistenceError::io("checking console state path", error)),
        }
    }
    Ok(())
}

pub(super) fn ensure_regular_file(path: &Path) -> Result<(), PersistenceError> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    configure_lock_sharing(&mut options);
    match options.open(path) {
        Ok(file) => {
            file.sync_all()
                .map_err(|error| PersistenceError::io("syncing writer lock", error))?;
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(PersistenceError::io("creating writer lock", error)),
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| PersistenceError::io("checking writer lock", error))?;
    reject_link_metadata(path, &metadata)?;
    if !metadata.is_file() {
        return Err(PersistenceError::corruption(
            "Gateway console writer lock is not a regular file",
        ));
    }
    Ok(())
}

#[cfg(windows)]
pub(super) fn configure_lock_sharing(options: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

    options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
}

#[cfg(not(windows))]
pub(super) fn configure_lock_sharing(_options: &mut OpenOptions) {}

pub(super) fn reject_link_metadata(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), PersistenceError> {
    if metadata.file_type().is_symlink() || is_reparse_point(metadata) {
        return Err(PersistenceError::new(
            "console_journal_path_escape",
            format!(
                "Gateway console persistence rejects symlink or reparse-point component '{}'",
                path.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(windows)]
pub(super) fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
pub(super) fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}
