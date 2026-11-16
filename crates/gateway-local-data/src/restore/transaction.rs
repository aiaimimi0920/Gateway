//! Rename-only publication journal; preserve pre-restore data and roll back interrupted batches.
use super::{checked, operation_lock, write_marker, UNITS};
use std::{
    fs, io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) fn apply_pending(root: &Path) -> io::Result<()> {
    let _guard = operation_lock(root)?;
    let pending = checked(root, "restore-pending")?;
    if !pending.exists() {
        return Ok(());
    }
    if !checked(&pending, "ready")?.exists() {
        return Err(io::Error::other("Restore staging is incomplete"));
    }
    let old = checked(&pending, "previous")?;
    let started = checked(&pending, "started")?;
    let committed = checked(&pending, "committed")?;
    if committed.exists() {
        return archive(root, &pending);
    }
    if started.exists() {
        rollback(root, &pending, &old)?;
        archive(root, &pending)?;
        return Err(io::Error::other(
            "Interrupted restore was rolled back; original data preserved. Restart Gateway again",
        ));
    }
    if old.exists() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        fs::rename(
            &old,
            checked(&pending, &format!("unused-preparation-{stamp}"))?,
        )?;
    }
    fs::create_dir(&old)?;
    // Record original presence before making any changes. A missing marker is not an absence.
    for unit in UNITS {
        let existed = checked(root, unit)?.exists();
        write_marker(
            &old.join(format!("{unit}.presence")),
            if existed { b"1" } else { b"0" },
        )?;
    }
    write_marker(&started, b"1")?;
    let result = (|| {
        for unit in UNITS {
            let current = checked(root, unit)?;
            if current.exists() {
                fs::rename(&current, checked(&old, unit)?)?;
            }
            let candidate = checked(&pending, unit)?;
            if candidate.exists() {
                fs::rename(candidate, current)?;
            }
        }
        write_marker(&committed, b"1")
    })();
    if result.is_err() {
        rollback(root, &pending, &old)?;
        archive(root, &pending)?;
        return Err(io::Error::other(
            "Restore failed and was rolled back; original data preserved",
        ));
    }
    archive(root, &pending)
}

fn rollback(root: &Path, pending: &Path, old: &Path) -> io::Result<()> {
    let displaced = checked(pending, "failed")?;
    fs::create_dir_all(&displaced)?;
    for unit in UNITS.into_iter().rev() {
        let previous = checked(old, unit)?;
        let current = checked(root, unit)?;
        let presence = fs::read(checked(old, &format!("{unit}.presence"))?)?;
        if previous.exists() {
            if current.exists() {
                fs::rename(&current, checked(&displaced, unit)?)?;
            }
            fs::rename(previous, current)?;
        } else if presence == b"0" && current.exists() {
            fs::rename(current, checked(&displaced, unit)?)?;
        } else if presence != b"0" && presence != b"1" {
            return Err(io::Error::other(
                "Invalid restore journal; recovery requires inspection",
            ));
        }
    }
    Ok(())
}

pub(super) fn archive(root: &Path, pending: &Path) -> io::Result<()> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let directory = checked(root, "backups")?;
    super::create_private_directory(&directory)?;
    fs::rename(pending, checked(&directory, &format!("restore-{stamp}"))?)
}
