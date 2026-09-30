//! Startup migrations are serialized, backed up, and recoverable after interruption.
//! Compatible reader changes must NOT bump this version or rewrite user data.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

pub const CURRENT_VERSION: u32 = 1;
const VERSION_FILE: &str = "storage-version";

struct Migration {
    from: u32,
    files: &'static [&'static str],
    apply: fn(&Path) -> io::Result<()>,
}

pub fn upgrade(root: &Path) -> io::Result<File> {
    // V1 adopts the existing YAML/local journal format without changing its bytes.
    run(
        root,
        CURRENT_VERSION,
        &[Migration {
            from: 0,
            files: &[],
            apply: |_| Ok(()),
        }],
    )
}

fn version(root: &Path) -> io::Result<u32> {
    let path = owned_file(root, VERSION_FILE)?;
    match File::open(path) {
        Ok(file) => {
            if file.metadata()?.len() > 64 {
                return Err(io::Error::other("Invalid Gateway storage version length"));
            }
            let mut text = String::new();
            file.take(64).read_to_string(&mut text)?;
            text.trim()
                .parse()
                .map_err(|_| io::Error::other("Invalid Gateway storage version"))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error),
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("pending");
    let mut file = File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, path)
}

fn atomic_copy(source: &Path, target: &Path) -> io::Result<()> {
    let mut input = File::open(source)?;
    let temporary = target.with_extension("pending");
    let mut output = File::create(&temporary)?;
    io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    drop(output);
    fs::rename(temporary, target)
}

fn owned_file(root: &Path, relative: &str) -> io::Result<PathBuf> {
    let mut path = root.to_path_buf();
    for part in Path::new(relative).components() {
        if !matches!(part, Component::Normal(_)) {
            return Err(io::Error::other(
                "Migration paths must stay inside the data directory",
            ));
        }
        path.push(part);
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if metadata.file_type().is_symlink() {
                return Err(io::Error::other("Migration target must not be a symlink"));
            }
        }
    }
    Ok(path)
}

fn snapshot(root: &Path, backup: &Path, files: &[&str]) -> io::Result<()> {
    fs::create_dir_all(backup)?;
    for (index, relative) in files.iter().enumerate() {
        let source = owned_file(root, relative)?;
        // An interrupted, uncommitted snapshot may contain an obsolete absence marker.
        match fs::remove_file(backup.join(format!("{index}.absent"))) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        match atomic_copy(&source, &backup.join(index.to_string())) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                atomic_write(&backup.join(format!("{index}.absent")), b"")?;
            }
            Err(error) => return Err(error),
        }
    }
    atomic_write(&backup.join("prepared"), b"1")
}

fn restore(root: &Path, backup: &Path, files: &[&str]) -> io::Result<()> {
    for (index, relative) in files.iter().enumerate() {
        let target = owned_file(root, relative)?;
        if backup.join(format!("{index}.absent")).exists() {
            match fs::remove_file(target) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            atomic_copy(&backup.join(index.to_string()), &target)?;
        }
    }
    Ok(())
}

fn run(root: &Path, current: u32, migrations: &[Migration]) -> io::Result<File> {
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("storage-migration.lock"))?;
    lock.try_lock().map_err(|_| {
        io::Error::other("Another Gateway process is migrating local data; retry startup")
    })?;
    let mut stored = version(root)?;
    if stored > current {
        return Err(io::Error::other(
            "Local data requires a newer Gateway; no data was modified",
        ));
    }
    // Both EXEs retain shared leases. Incompatible upgrades require every older
    // reader/writer to exit before transforming files or database snapshots.
    let usage = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("storage-use.lock"))?;
    let upgrading = stored < current;
    if upgrading {
        usage.try_lock().map_err(|_| {
            io::Error::other("Close other Gateway instances before upgrading local data")
        })?;
    }
    while stored < current {
        let step = migrations
            .iter()
            .find(|step| step.from == stored)
            .ok_or_else(|| {
                io::Error::other("No supported migration for this Gateway data version")
            })?;
        let backup = owned_file(root, &format!("migrations/v{stored}-to-v{}", stored + 1))?;
        if backup.join("prepared").exists() {
            // A previous attempt did not advance the version: undo its partial writes.
            restore(root, &backup, step.files)?;
        } else {
            snapshot(root, &backup, step.files)?;
        }
        if let Err(error) = (step.apply)(root) {
            restore(root, &backup, step.files)?;
            return Err(error);
        }
        stored += 1;
        atomic_write(
            &owned_file(root, VERSION_FILE)?,
            format!("{stored}\n").as_bytes(),
        )?;
    }
    if upgrading {
        usage.unlock()?;
    }
    usage.lock_shared()?;
    Ok(usage)
}

#[cfg(test)]
mod tests;
