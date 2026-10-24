//! Fixed-owner file access. Requests never select filesystem paths; reads stay bounded as logs grow.
use super::super::ConsoleConfig;
use std::{
    fs,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub const MAX_ENTRIES: usize = 4096;
pub const MAX_FILE_BYTES: u64 = 256 * 1024;

pub fn reparse(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

pub fn absolute(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

pub fn validate_handle(file: &fs::File) -> io::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || reparse(&metadata) {
        return Err(io::Error::other("Unsafe Gateway log handle"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };
        let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        // The OS fills this fixed-size record only on success; the live File owns the handle.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { info.assume_init() }.nNumberOfLinks != 1 {
            return Err(io::Error::other("Hard-linked Gateway log"));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(io::Error::other("Hard-linked Gateway log"));
        }
    }
    Ok(())
}

pub fn reject_links(path: &Path) -> io::Result<()> {
    let path = absolute(path)?;
    for component in path.ancestors() {
        match fs::symlink_metadata(component) {
            Ok(metadata) if reparse(&metadata) => {
                return Err(io::Error::other("Redirected log path"))
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub fn ensure_directory(path: &Path) -> Result<(), String> {
    reject_links(path).map_err(|_| "Unsafe Gateway log directory")?;
    fs::create_dir_all(path).map_err(|_| "Cannot create Gateway log directory")?;
    reject_links(path).map_err(|_| "Unsafe Gateway log directory".to_owned())
}

pub fn directory(config: &ConsoleConfig) -> Result<PathBuf, String> {
    let path = if let Some(source) = &config.log_source {
        absolute(source)
            .map_err(|_| "Cannot resolve Gateway log source")?
            .parent()
            .ok_or("Invalid Gateway log source")?
            .to_path_buf()
    } else {
        absolute(&config.state_dir.join("logs")).map_err(|_| "Cannot resolve Gateway logs")?
    };
    reject_links(&path).map_err(|_| "Unsafe Gateway log directory")?;
    Ok(path)
}

pub fn open_regular(path: &Path) -> io::Result<fs::File> {
    reject_links(path)?;
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() || reparse(&before) {
        return Err(io::Error::other("Not a regular log file"));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // Open the reparse point itself, not its target.
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    validate_handle(&file)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || reparse(&metadata) {
        return Err(io::Error::other("Unsafe Gateway log handle"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 || before.dev() != metadata.dev() || before.ino() != metadata.ino()
        {
            return Err(io::Error::other("Gateway log identity changed"));
        }
    }
    reject_links(path)?;
    Ok(file)
}

pub fn read_small(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    open_regular(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::other("Gateway file exceeds limit"));
    }
    Ok(bytes)
}

pub fn owned_name(name: &str) -> bool {
    (name.starts_with("gateway-runtime-") || name.starts_with("gateway-v2-"))
        && name.ends_with(".log")
}

pub fn logs(config: &ConsoleConfig) -> Result<Vec<PathBuf>, String> {
    let directory = directory(config)?;
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err("Cannot list Gateway logs".into()),
    };
    let current = config
        .log_source
        .as_ref()
        .and_then(|path| absolute(path).ok());
    let mut paths = Vec::new();
    for (index, entry) in entries.enumerate() {
        if index >= MAX_ENTRIES {
            return Err("Too many Gateway log directory entries".into());
        }
        let entry = entry.map_err(|_| "Cannot inspect Gateway logs")?;
        if !owned_name(&entry.file_name().to_string_lossy())
            && current.as_ref() != Some(&entry.path())
        {
            continue;
        }
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| "Cannot inspect Gateway log")?;
        if metadata.is_file() && !reparse(&metadata) {
            paths.push(entry.path());
        }
    }
    paths.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    paths.reverse();
    Ok(paths)
}

pub fn tail(path: &Path) -> io::Result<(String, bool)> {
    let mut file = open_regular(path)?;
    let length = file.metadata()?.len();
    let start = length.saturating_sub(MAX_FILE_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(length - start).read_to_end(&mut bytes)?;
    // Drop a partial leading record: otherwise a truncated secret can lose its identifying key.
    if start > 0 {
        match bytes.iter().position(|byte| *byte == b'\n') {
            Some(index) => {
                bytes.drain(..=index);
            }
            None => bytes.clear(),
        }
    }
    Ok((String::from_utf8_lossy(&bytes).into_owned(), start > 0))
}
