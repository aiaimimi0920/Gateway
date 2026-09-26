//! Validated file copy and immutable snapshot publication.

use super::*;

fn collect_directory_files(root: &Path) -> Result<BTreeSet<String>, String> {
    fn visit(root: &Path, directory: &Path, files: &mut BTreeSet<String>) -> Result<(), String> {
        let entries = fs::read_dir(directory).map_err(|error| {
            format!(
                "failed to inspect web UI snapshot directory {}: {error}",
                directory.display()
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "failed to inspect web UI snapshot entry below {}: {error}",
                    directory.display()
                )
            })?;
            let file_type = entry.file_type().map_err(|error| {
                format!(
                    "failed to inspect web UI snapshot entry {}: {error}",
                    entry.path().display()
                )
            })?;
            let entry_path = entry.path();
            if file_type.is_dir() {
                visit(root, &entry_path, files)?;
            } else if file_type.is_file() {
                let relative_path = entry_path.strip_prefix(root).map_err(|error| {
                    format!(
                        "failed to make web UI snapshot path {} relative to {}: {error}",
                        entry_path.display(),
                        root.display()
                    )
                })?;
                files.insert(relative_path.to_string_lossy().replace('\\', "/"));
            } else {
                return Err(format!(
                    "web UI snapshot contains unsupported filesystem entry: {}",
                    entry_path.display()
                ));
            }
        }
        Ok(())
    }

    let mut files = BTreeSet::new();
    visit(root, root, &mut files)?;
    Ok(files)
}

fn verify_snapshot_manifest(
    snapshot_root: &Path,
    files: &[ValidatedWebFile],
) -> Result<(), String> {
    let expected_files = files
        .iter()
        .map(|file| (file.relative_path.clone(), file.expected_digest.as_str()))
        .collect::<BTreeMap<_, _>>();
    let actual_files = collect_directory_files(snapshot_root)?;
    let expected_paths = expected_files.keys().cloned().collect::<BTreeSet<_>>();
    if actual_files != expected_paths {
        return Err(format!(
            "web UI snapshot {} does not exactly match the ready marker manifest; expected {expected_paths:?}, found {actual_files:?}",
            snapshot_root.display()
        ));
    }

    for (relative_path, expected_digest) in expected_files {
        let snapshot_path = snapshot_root.join(safe_relative_path(&relative_path)?);
        let contents = fs::read(&snapshot_path).map_err(|error| {
            format!(
                "failed to read copied web UI snapshot file {}: {error}",
                snapshot_path.display()
            )
        })?;
        let actual_digest = sha256_hex(contents);
        if actual_digest != expected_digest {
            return Err(format!(
                "copied web UI snapshot file {relative_path} has digest {actual_digest}, expected {expected_digest}"
            ));
        }
    }
    Ok(())
}

pub(super) fn copy_validated_files_to_directory<F>(
    files: &[ValidatedWebFile],
    destination_root: &Path,
    mut copy_file: F,
) -> Result<(), String>
where
    F: FnMut(&Path, &Path) -> Result<(), String>,
{
    if destination_root.exists() {
        return Err(format!(
            "web UI snapshot staging directory already exists: {}",
            destination_root.display()
        ));
    }
    fs::create_dir_all(destination_root).map_err(|error| {
        format!(
            "failed to create web UI snapshot staging directory {}: {error}",
            destination_root.display()
        )
    })?;

    for file in files {
        let destination = destination_root.join(safe_relative_path(&file.relative_path)?);
        let parent = destination.parent().ok_or_else(|| {
            format!(
                "web UI snapshot file has no parent directory: {}",
                destination.display()
            )
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create web UI snapshot directory {}: {error}",
                parent.display()
            )
        })?;
        copy_file(&file.source_path, &destination).map_err(|error| {
            format!(
                "failed to copy published web UI file {} to snapshot {}: {error}",
                file.source_path.display(),
                destination.display()
            )
        })?;

        let copied_contents = fs::read(&destination).map_err(|error| {
            format!(
                "failed to re-read copied web UI snapshot file {}: {error}",
                destination.display()
            )
        })?;
        let copied_digest = sha256_hex(copied_contents);
        if copied_digest != file.expected_digest {
            return Err(format!(
                "copied web UI snapshot file {} has digest {copied_digest}, expected {}",
                file.relative_path, file.expected_digest
            ));
        }
    }

    verify_snapshot_manifest(destination_root, files)
}

fn remove_path_if_present(path: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "failed to inspect existing web UI snapshot {}: {error}",
                path.display()
            ));
        }
    };
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .map_err(|error| format!("failed to remove {}: {error}", path.display()))
}

pub(super) fn publish_web_ui_snapshot(
    snapshot_root: &Path,
    published: &ValidatedWebUi,
) -> Result<(), String> {
    let snapshot_parent = snapshot_root.parent().ok_or_else(|| {
        format!(
            "web UI snapshot path has no parent directory: {}",
            snapshot_root.display()
        )
    })?;
    fs::create_dir_all(snapshot_parent).map_err(|error| {
        format!(
            "failed to create web UI snapshot parent {}: {error}",
            snapshot_parent.display()
        )
    })?;
    let staging_root = snapshot_parent.join(format!(
        ".{WEB_UI_SNAPSHOT_DIR_NAME}.staging-{}-{}",
        std::process::id(),
        unix_time_nanos()?
    ));

    let copy_result = copy_validated_files_to_directory(
        &published.files,
        &staging_root,
        |source, destination| {
            fs::copy(source, destination)
                .map(|_| ())
                .map_err(|error| error.to_string())
        },
    );
    if let Err(error) = copy_result {
        let _ = remove_path_if_present(&staging_root);
        return Err(error);
    }

    if let Err(error) = remove_path_if_present(snapshot_root) {
        let _ = remove_path_if_present(&staging_root);
        return Err(error);
    }
    if let Err(error) = fs::rename(&staging_root, snapshot_root) {
        let _ = remove_path_if_present(&staging_root);
        return Err(format!(
            "failed to publish immutable web UI snapshot {}: {error}",
            snapshot_root.display()
        ));
    }
    verify_snapshot_manifest(snapshot_root, &published.files)
}
