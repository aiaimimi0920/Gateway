//! Resolve lexical and physical paths before enforcing release containment.

#[cfg(windows)]
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::ConsoleConfigError;

pub fn validate_state_directory(
    state_dir: &Path,
    release_payload_root: Option<&Path>,
) -> Result<(), ConsoleConfigError> {
    let Some(release_payload_root) = release_payload_root else {
        return Ok(());
    };

    let state_paths = resolve_path_views(state_dir)?;
    let release_paths = resolve_path_views(release_payload_root)?;
    validate_state_path_views(
        &state_paths,
        &release_paths,
        state_dir,
        release_payload_root,
    )
}

pub(super) fn validate_state_path_views(
    state: &ResolvedPathViews,
    release: &ResolvedPathViews,
    state_dir: &Path,
    release_payload_root: &Path,
) -> Result<(), ConsoleConfigError> {
    if path_views_are_same_or_descendant(state, release) {
        return Err(ConsoleConfigError::new(
            "console_state_inside_release_payload",
            format!(
                "Gateway console state directory '{}' must be outside release payload '{}'",
                state_dir.display(),
                release_payload_root.display()
            ),
        ));
    }
    if path_views_are_same_or_descendant(release, state) {
        return Err(ConsoleConfigError::new(
            "console_state_contains_release_payload",
            format!(
                "Gateway console state directory '{}' must not contain release payload '{}'",
                state_dir.display(),
                release_payload_root.display()
            ),
        ));
    }

    Ok(())
}

pub(super) fn validate_routes_path_views(
    routes: &ResolvedPathViews,
    release: &ResolvedPathViews,
    routes_file: &Path,
    release_payload_root: &Path,
) -> Result<(), ConsoleConfigError> {
    if path_views_are_same_or_descendant(routes, release) {
        return Err(ConsoleConfigError::new(
            "console_routes_inside_release_payload",
            format!(
                "Gateway routes file '{}' must be outside release payload '{}'",
                routes_file.display(),
                release_payload_root.display()
            ),
        ));
    }

    Ok(())
}

#[derive(Debug)]
pub(super) struct ResolvedPathViews {
    pub(super) lexical: PathBuf,
    physical: PathBuf,
}

pub(super) fn resolve_path_views(path: &Path) -> Result<ResolvedPathViews, ConsoleConfigError> {
    let absolute = std::path::absolute(path).map_err(|error| path_resolution_error(path, error))?;

    Ok(ResolvedPathViews {
        lexical: normalize_lexically(&absolute),
        physical: resolve_physical_path(&absolute)?,
    })
}

#[cfg(windows)]
fn resolve_physical_path(path: &Path) -> Result<PathBuf, ConsoleConfigError> {
    resolve_existing_ancestor(&normalize_lexically(path))
}

#[cfg(not(windows))]
fn resolve_physical_path(path: &Path) -> Result<PathBuf, ConsoleConfigError> {
    let mut resolved = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                resolved.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(value) => {
                let candidate = resolved.join(value);
                match fs::symlink_metadata(&candidate) {
                    Ok(_) => {
                        resolved = fs::canonicalize(&candidate)
                            .map_err(|error| path_resolution_error(path, error))?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        resolved.push(value);
                    }
                    Err(error) => return Err(path_resolution_error(path, error)),
                }
            }
        }
    }

    Ok(resolved)
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
        }
    }

    normalized
}

#[cfg(windows)]
fn resolve_existing_ancestor(path: &Path) -> Result<PathBuf, ConsoleConfigError> {
    let mut ancestor = path.to_path_buf();
    let mut missing_components = Vec::<OsString>::new();

    loop {
        match fs::symlink_metadata(&ancestor) {
            Ok(metadata) => {
                let mut resolved = canonicalize_existing_path(&ancestor, &metadata)
                    .map_err(|error| path_resolution_error(path, error))?;
                for component in missing_components.iter().rev() {
                    resolved.push(component);
                }
                return Ok(normalize_lexically(&resolved));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(component) = ancestor.file_name().map(OsString::from) else {
                    return Err(path_resolution_error(path, error));
                };
                missing_components.push(component);
                if !ancestor.pop() {
                    return Err(path_resolution_error(path, error));
                }
            }
            Err(error) => return Err(path_resolution_error(path, error)),
        }
    }
}

#[cfg(windows)]
fn canonicalize_existing_path(path: &Path, metadata: &fs::Metadata) -> std::io::Result<PathBuf> {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return fs::canonicalize(path);
    }

    fs::canonicalize(path)
}

fn path_resolution_error(path: &Path, error: std::io::Error) -> ConsoleConfigError {
    ConsoleConfigError::new(
        "console_path_resolution_failed",
        format!(
            "Failed to resolve Gateway console path '{}': {}",
            path.display(),
            error
        ),
    )
}

fn path_views_are_same_or_descendant(path: &ResolvedPathViews, parent: &ResolvedPathViews) -> bool {
    path_is_same_or_descendant(&path.lexical, &parent.lexical)
        || path_is_same_or_descendant(&path.physical, &parent.physical)
}

#[cfg(not(windows))]
fn path_is_same_or_descendant(path: &Path, parent: &Path) -> bool {
    path.starts_with(parent)
}

#[cfg(windows)]
fn path_is_same_or_descendant(path: &Path, parent: &Path) -> bool {
    let mut path_components = path
        .components()
        .filter(|component| !matches!(component, Component::CurDir));

    parent
        .components()
        .filter(|component| !matches!(component, Component::CurDir))
        .all(|parent_component| {
            path_components.next().is_some_and(|path_component| {
                windows_components_equal(path_component, parent_component)
            })
        })
}

#[cfg(windows)]
fn windows_components_equal(left: Component<'_>, right: Component<'_>) -> bool {
    match (left, right) {
        (Component::Prefix(left), Component::Prefix(right)) => {
            windows_prefixes_equal(left.kind(), right.kind())
        }
        (Component::RootDir, Component::RootDir)
        | (Component::CurDir, Component::CurDir)
        | (Component::ParentDir, Component::ParentDir) => true,
        (Component::Normal(left), Component::Normal(right)) => {
            windows_os_str_eq_ignore_case(left, right)
        }
        _ => false,
    }
}

#[cfg(windows)]
fn windows_prefixes_equal(left: std::path::Prefix<'_>, right: std::path::Prefix<'_>) -> bool {
    use std::path::Prefix;

    match (left, right) {
        (
            Prefix::Disk(left) | Prefix::VerbatimDisk(left),
            Prefix::Disk(right) | Prefix::VerbatimDisk(right),
        ) => left.eq_ignore_ascii_case(&right),
        (
            Prefix::UNC(left_server, left_share) | Prefix::VerbatimUNC(left_server, left_share),
            Prefix::UNC(right_server, right_share) | Prefix::VerbatimUNC(right_server, right_share),
        ) => {
            windows_os_str_eq_ignore_case(left_server, right_server)
                && windows_os_str_eq_ignore_case(left_share, right_share)
        }
        (Prefix::DeviceNS(left), Prefix::DeviceNS(right))
        | (Prefix::Verbatim(left), Prefix::Verbatim(right)) => {
            windows_os_str_eq_ignore_case(left, right)
        }
        _ => false,
    }
}

#[cfg(windows)]
fn windows_os_str_eq_ignore_case(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Globalization::{CompareStringOrdinal, CSTR_EQUAL};

    let left = left.encode_wide().collect::<Vec<_>>();
    let right = right.encode_wide().collect::<Vec<_>>();
    let (Ok(left_len), Ok(right_len)) = (i32::try_from(left.len()), i32::try_from(right.len()))
    else {
        return false;
    };

    // Counted UTF-16 comparison preserves unpaired surrogates in Windows paths.
    unsafe {
        CompareStringOrdinal(left.as_ptr(), left_len, right.as_ptr(), right_len, 1) == CSTR_EQUAL
    }
}
