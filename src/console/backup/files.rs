//! Bounded collection of owned data, rejecting links rather than following them out of the root.
use gateway_local_data::restore::{checked, MAX_BYTES};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::Path,
};

pub(super) fn collect(root: &Path, scratch: &Path) -> io::Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut total = 0;
    for unit in gateway_local_data::restore::UNITS {
        walk(root, unit, scratch, &mut files, &mut total)?;
    }
    Ok(files)
}

fn walk(
    root: &Path,
    name: &str,
    scratch: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
    total: &mut usize,
) -> io::Result<()> {
    // Only ordinary history/temporary files are excluded; monetary and recovery journals remain.
    if name == "local/state/runtime.sqlite3"
        || name.starts_with("local/state/runtime.sqlite3-")
        || name == "local/state/logs"
        || name == "local/logs"
        || name.ends_with(".lock")
        || name.contains("/.backup-")
        || name
            .rsplit('/')
            .next()
            .is_some_and(|p| p.starts_with('.') && p.contains(".tmp"))
    {
        return Ok(());
    }
    let path = checked(root, name)?;
    let metadata = match fs::metadata(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.is_dir() {
        if name.split('/').count() > 32 {
            return Err(io::Error::other("Backup directory depth exceeded"));
        }
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let child = entry
                .file_name()
                .into_string()
                .map_err(|_| io::Error::other("Backup contains a non-UTF-8 name"))?;
            walk(root, &format!("{name}/{child}"), scratch, files, total)?;
        }
    } else if metadata.is_file() {
        if super::sqlite_files::is_sidecar(&path)? {
            return Ok(());
        }
        if files.len() >= 4096 {
            return Err(io::Error::other("Backup file count exceeded"));
        }
        if super::sqlite_files::is_database(&path)? {
            let bytes =
                super::sqlite_files::snapshot(&path, scratch, MAX_BYTES.saturating_sub(*total))?;
            *total += bytes.len();
            files.insert(name.to_owned(), bytes);
            return Ok(());
        }
        let mut bytes = Vec::new();
        fs::File::open(&path)?
            .take((MAX_BYTES.saturating_sub(*total) + 1) as u64)
            .read_to_end(&mut bytes)?;
        *total += bytes.len();
        if *total > MAX_BYTES {
            return Err(io::Error::other("Backup data exceeds 32 MiB"));
        }
        // Refuse a concurrently replaced/modified file; callers also hold the route writer lock.
        let after = fs::metadata(checked(root, name)?)?;
        if metadata.len() != after.len() || metadata.modified()? != after.modified()? {
            return Err(io::Error::other(
                "Backup source changed; retry when Gateway is idle",
            ));
        }
        files.insert(name.to_owned(), bytes);
    } else {
        return Err(io::Error::other("Backup contains a special file"));
    }
    Ok(())
}
