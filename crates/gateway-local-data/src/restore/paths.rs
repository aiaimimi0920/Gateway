//! Strict archive namespace, size budget and link/reparse-point rejection on all traversed paths.
use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};
pub const MAX_BYTES: usize = 32 * 1024 * 1024;

pub fn checked(root: &Path, relative: &str) -> io::Result<PathBuf> {
    let mut path = root.to_owned();
    reject_link(&path)?;
    for part in relative.split('/') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with([' ', '.'])
            || part
                .chars()
                .any(|c| c.is_control() || "\\:*?\"<>|".contains(c))
            || ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.as_bytes()[3].is_ascii_digit())
        {
            return Err(io::Error::other("Unsafe backup path"));
        }
        path.push(part);
        reject_link(&path)?;
    }
    Ok(path)
}

fn reject_link(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err(io::Error::other(
                        "Restore paths cannot contain reparse points",
                    ));
                }
            }
            if metadata.file_type().is_symlink() {
                return Err(io::Error::other("Restore paths cannot contain links"));
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn validate_files(files: &BTreeMap<String, Vec<u8>>) -> io::Result<()> {
    if files.is_empty() || files.len() > 4096 {
        return Err(io::Error::other("Unsupported backup file count"));
    }
    for required in [
        "local/routes.yaml",
        "local/state/runtime.sqlite3",
        "local/state/access-key-seal.json",
    ] {
        if !files.contains_key(required) {
            return Err(io::Error::other("Backup is missing required data"));
        }
    }
    let mut total = 0usize;
    let mut names = HashSet::new();
    for (name, bytes) in files {
        // Lexical validation must not depend on the process working directory.
        if name.len() > 1024
            || !super::UNITS.iter().any(|unit| {
                name == unit || (unit != &"gateway.json" && name.starts_with(&format!("{unit}/")))
            })
        {
            return Err(io::Error::other("Backup contains an unsupported data path"));
        }
        let lower = name.to_lowercase();
        if !names.insert(lower) {
            return Err(io::Error::other("Backup paths conflict on Windows"));
        }
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("Backup size overflow"))?;
        if total > MAX_BYTES {
            return Err(io::Error::other("Backup exceeds size limit"));
        }
        // Use an absolute non-existent suffix only for lexical checks; actual staging validates again.
        for part in name.split('/') {
            if part.is_empty()
                || part == "."
                || part == ".."
                || part
                    .chars()
                    .any(|c| c.is_control() || "\\:*?\"<>|".contains(c))
                || part.ends_with([' ', '.'])
            {
                return Err(io::Error::other("Unsafe backup path"));
            }
        }
    }
    for name in files.keys() {
        let mut prefix = String::new();
        for part in name.split('/').take(name.split('/').count() - 1) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            if names.contains(&prefix.to_lowercase()) {
                return Err(io::Error::other("Backup file conflicts with a directory"));
            }
        }
    }
    Ok(())
}
