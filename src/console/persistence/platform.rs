//! Native atomic replacement preserves platform flags, path buffers and backup preconditions.

#[cfg(not(windows))]
use super::file_io::{create_new_bytes_io, read_regular_file_io};
use super::paths::is_reparse_point;
use super::{
    AtomicReplaceBackend, PlatformAtomicReplaceBackend, RouteConfigPersistence,
    MOVEFILE_WRITE_THROUGH_FLAG, REPLACE_FILE_FLAGS,
};
use std::path::Path;
use std::{fs, io};

#[cfg(windows)]
impl AtomicReplaceBackend for PlatformAtomicReplaceBackend {
    fn replace_existing(
        &self,
        destination: &Path,
        replacement: &Path,
        backup: &Path,
        flags: u32,
    ) -> io::Result<()> {
        use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

        if flags != REPLACE_FILE_FLAGS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ReplaceFileW flags must be zero",
            ));
        }

        let destination = windows_path(destination)?;
        let replacement = windows_path(replacement)?;
        let backup = windows_path(backup)?;
        let result = unsafe {
            ReplaceFileW(
                destination.as_ptr(),
                replacement.as_ptr(),
                backup.as_ptr(),
                flags,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn move_new(&self, source: &Path, destination: &Path, flags: u32) -> io::Result<()> {
        use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

        if flags != MOVEFILE_WRITE_THROUGH_FLAG {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MoveFileExW must use MOVEFILE_WRITE_THROUGH without REPLACE_EXISTING",
            ));
        }

        let source = windows_path(source)?;
        let destination = windows_path(destination)?;
        let result = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) };
        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(not(windows))]
impl AtomicReplaceBackend for PlatformAtomicReplaceBackend {
    fn replace_existing(
        &self,
        destination: &Path,
        replacement: &Path,
        _backup: &Path,
        flags: u32,
    ) -> io::Result<()> {
        if flags != REPLACE_FILE_FLAGS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "POSIX replacement flags must be zero",
            ));
        }
        fs::rename(replacement, destination)
    }

    fn move_new(&self, source: &Path, destination: &Path, flags: u32) -> io::Result<()> {
        if flags != MOVEFILE_WRITE_THROUGH_FLAG {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "POSIX new-file move flags must be MOVEFILE_WRITE_THROUGH",
            ));
        }
        fs::rename(source, destination)
    }
}

#[cfg(windows)]
fn windows_path(path: &Path) -> io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;

    let raw = path.as_os_str().encode_wide().collect::<Vec<_>>();
    let mut encoded = if raw.starts_with(&[b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16]) {
        raw
    } else if raw.starts_with(&[b'\\' as u16, b'\\' as u16]) {
        let mut value = r"\\?\UNC\".encode_utf16().collect::<Vec<_>>();
        value.extend_from_slice(&raw[2..]);
        value
    } else {
        let mut value = r"\\?\".encode_utf16().collect::<Vec<_>>();
        value.extend_from_slice(&raw);
        value
    };
    if encoded.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows path contains an interior NUL",
        ));
    }
    encoded.push(0);
    Ok(encoded)
}

impl RouteConfigPersistence {
    pub(super) fn replace_existing_with_backup(
        &self,
        destination: &Path,
        replacement: &Path,
        backup: &Path,
    ) -> io::Result<()> {
        if destination.parent() != replacement.parent() || destination.parent() != backup.parent() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "atomic replacement files must share one parent directory",
            ));
        }
        match fs::symlink_metadata(backup) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "atomic replacement backup path is a link/reparse point",
                    ));
                }
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "atomic replacement backup path already exists",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        #[cfg(not(windows))]
        {
            let previous = read_regular_file_io(destination)?;
            create_new_bytes_io(backup, &previous)?;
        }
        self.backend
            .replace_existing(destination, replacement, backup, REPLACE_FILE_FLAGS)
    }
}
