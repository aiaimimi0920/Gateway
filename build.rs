use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Component, Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const PREBUILT_WEB_UI_ENV: &str = "GATEWAY_PREBUILT_WEB_UI";
const WEB_UI_SNAPSHOT_ENV: &str = "GATEWAY_WEB_UI_SNAPSHOT";
const WEB_UI_SNAPSHOT_DIR_NAME: &str = "gateway-web-ui-snapshot";
const GENERATED_EMBED_SOURCE_NAME: &str = "gateway_embedded_ui.rs";
const DIST_WEB_ROOT_PATH: &str = "apps/desktop/dist/web";
const DIST_WEB_INDEX_PATH: &str = "apps/desktop/dist/web/index.html";
const DIST_WEB_READY_PATH: &str = "apps/desktop/dist/.gateway-web-ready";
const DIST_WEB_PUBLISH_LOCK_PATH: &str = "apps/desktop/dist/.gateway-web-publish.lock";
const DIST_WEB_PUBLISH_LOCK_OWNER_PATH: &str =
    "apps/desktop/dist/.gateway-web-publish.lock/owner.json";
const READY_MARKER_SCHEMA_VERSION: u64 = 1;
const PREBUILT_WEB_UI_WAIT_ATTEMPTS: usize = 100;
const PREBUILT_WEB_UI_WAIT_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug)]
struct ValidatedWebFile {
    relative_path: String,
    source_path: PathBuf,
    expected_digest: String,
}

#[derive(Debug)]
struct ValidatedWebUi {
    marker_text: String,
    files: Vec<ValidatedWebFile>,
}

struct PublishValidationLock {
    owner_token: String,
    released: bool,
}

impl PublishValidationLock {
    fn acquire() -> Result<Self, String> {
        let mut attempted_recovery = false;
        loop {
            match fs::create_dir(DIST_WEB_PUBLISH_LOCK_PATH) {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if !attempted_recovery && recover_stale_publish_lock()? {
                        attempted_recovery = true;
                        continue;
                    }
                    return Err(format!(
                        "web publish lock is active: {DIST_WEB_PUBLISH_LOCK_PATH}"
                    ));
                }
                Err(error) => {
                    return Err(format!(
                        "failed to acquire web publish lock {DIST_WEB_PUBLISH_LOCK_PATH}: {error}"
                    ));
                }
            }
        }

        let acquired_at = unix_time_nanos()?;
        let owner_token = format!("build-{}-{acquired_at}", std::process::id());
        let owner = serde_json::json!({
            "ownerToken": owner_token,
            "pid": std::process::id(),
            "acquiredAtUnixNanos": acquired_at.to_string(),
            "purpose": "web-ui-snapshot",
        });
        if let Err(error) = fs::write(DIST_WEB_PUBLISH_LOCK_OWNER_PATH, format!("{owner}\n")) {
            let _ = fs::remove_dir_all(DIST_WEB_PUBLISH_LOCK_PATH);
            return Err(format!(
                "failed to write web publish lock owner {DIST_WEB_PUBLISH_LOCK_OWNER_PATH}: {error}"
            ));
        }

        Ok(Self {
            owner_token,
            released: false,
        })
    }

    fn release(mut self) -> Result<(), String> {
        self.release_owned_lock()?;
        self.released = true;
        Ok(())
    }

    fn release_owned_lock(&self) -> Result<(), String> {
        let owner = fs::read_to_string(DIST_WEB_PUBLISH_LOCK_OWNER_PATH).map_err(|error| {
            format!(
                "failed to read web publish lock owner {DIST_WEB_PUBLISH_LOCK_OWNER_PATH}: {error}"
            )
        })?;
        let owner: serde_json::Value = serde_json::from_str(&owner).map_err(|error| {
            format!(
                "failed to parse web publish lock owner {DIST_WEB_PUBLISH_LOCK_OWNER_PATH}: {error}"
            )
        })?;
        let current_owner = owner
            .get("ownerToken")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if current_owner != self.owner_token {
            return Err(format!(
                "web publish lock owner changed before release: {DIST_WEB_PUBLISH_LOCK_PATH}"
            ));
        }
        fs::remove_dir_all(DIST_WEB_PUBLISH_LOCK_PATH).map_err(|error| {
            format!("failed to release web publish lock {DIST_WEB_PUBLISH_LOCK_PATH}: {error}")
        })
    }
}

impl Drop for PublishValidationLock {
    fn drop(&mut self) {
        if !self.released {
            let _ = self.release_owned_lock();
        }
    }
}

fn unix_time_nanos() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))
}

fn recover_stale_publish_lock() -> Result<bool, String> {
    let recovered = recover_stale_publish_lock_with(process_is_alive)?;
    if recovered {
        println!(
            "cargo:warning=reclaimed a stale web publish lock after proving its owner process no longer exists"
        );
    }
    Ok(recovered)
}

fn recover_stale_publish_lock_with<F>(is_process_alive: F) -> Result<bool, String>
where
    F: FnOnce(u32) -> Result<bool, String>,
{
    // Never infer staleness from timestamps. Recovery requires an intact owner token,
    // a parseable PID, and a conclusive OS-level answer that the process exited.
    let owner_text = match fs::read_to_string(DIST_WEB_PUBLISH_LOCK_OWNER_PATH) {
        Ok(owner_text) => owner_text,
        Err(_) => return Ok(false),
    };
    let owner: serde_json::Value = match serde_json::from_str(&owner_text) {
        Ok(owner) => owner,
        Err(_) => return Ok(false),
    };
    let owner_token = match owner
        .get("ownerToken")
        .and_then(serde_json::Value::as_str)
        .filter(|owner_token| !owner_token.trim().is_empty())
    {
        Some(owner_token) => owner_token,
        None => return Ok(false),
    };
    let owner_pid = match owner
        .get("pid")
        .and_then(serde_json::Value::as_u64)
        .and_then(|pid| u32::try_from(pid).ok())
        .filter(|pid| *pid != 0)
    {
        Some(owner_pid) => owner_pid,
        None => return Ok(false),
    };

    let owner_is_alive = match is_process_alive(owner_pid) {
        Ok(owner_is_alive) => owner_is_alive,
        Err(_) => return Ok(false),
    };
    if owner_is_alive {
        return Ok(false);
    }

    let owner_after_probe = match fs::read_to_string(DIST_WEB_PUBLISH_LOCK_OWNER_PATH) {
        Ok(owner_after_probe) => owner_after_probe,
        Err(_) => return Ok(false),
    };
    if owner_after_probe != owner_text {
        return Ok(false);
    }

    // Rename first so cleanup can never remove a new publisher lock created at the
    // canonical path after this stale owner is detached.
    let quarantine_path = PathBuf::from(format!(
        "{DIST_WEB_PUBLISH_LOCK_PATH}.stale-{}-{}",
        std::process::id(),
        unix_time_nanos()?
    ));
    match fs::rename(DIST_WEB_PUBLISH_LOCK_PATH, &quarantine_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Ok(false),
    }

    let quarantined_owner = fs::read_to_string(quarantine_path.join("owner.json")).map_err(
        |error| {
            format!(
                "reclaimed stale web publish lock for owner {owner_token} (pid {owner_pid}), but failed to verify its quarantined owner: {error}"
            )
        },
    )?;
    if quarantined_owner != owner_text {
        return Err(format!(
            "web publish lock owner changed while reclaiming stale owner {owner_token} (pid {owner_pid})"
        ));
    }
    fs::remove_dir_all(&quarantine_path).map_err(|error| {
        format!(
            "reclaimed stale web publish lock for owner {owner_token} (pid {owner_pid}), but failed to remove {}: {error}",
            quarantine_path.display()
        )
    })?;
    Ok(true)
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> Result<bool, String> {
    use std::ffi::c_void;

    type Handle = *mut c_void;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    const ERROR_ACCESS_DENIED: u32 = 5;
    const ERROR_INVALID_PARAMETER: u32 = 87;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> Handle;
        fn GetExitCodeProcess(process: Handle, exit_code: *mut u32) -> i32;
        fn CloseHandle(object: Handle) -> i32;
        fn GetLastError() -> u32;
    }

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        let error = unsafe { GetLastError() };
        return match error {
            ERROR_INVALID_PARAMETER => Ok(false),
            ERROR_ACCESS_DENIED => Ok(true),
            _ => Err(format!(
                "failed to inspect web publish lock owner process {pid}: Windows error {error}"
            )),
        };
    }

    let mut exit_code = 0_u32;
    let query_result = unsafe { GetExitCodeProcess(process, &mut exit_code) };
    let query_error = if query_result == 0 {
        Some(unsafe { GetLastError() })
    } else {
        None
    };
    let _ = unsafe { CloseHandle(process) };
    if let Some(error) = query_error {
        return Err(format!(
            "failed to query web publish lock owner process {pid}: Windows error {error}"
        ));
    }
    Ok(exit_code == STILL_ACTIVE)
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> Result<bool, String> {
    const EPERM: i32 = 1;
    const ESRCH: i32 = 3;

    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }

    let Ok(pid) = i32::try_from(pid) else {
        return Ok(false);
    };
    if unsafe { kill(pid, 0) } == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(ESRCH) => Ok(false),
        Some(EPERM) => Ok(true),
        _ => Err(format!(
            "failed to inspect web publish lock owner process {pid}: {error}"
        )),
    }
}

#[cfg(not(any(unix, windows)))]
fn process_is_alive(pid: u32) -> Result<bool, String> {
    Err(format!(
        "cannot prove whether web publish lock owner process {pid} exists on this platform"
    ))
}

fn sha256_hex(contents: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(contents.as_ref()))
}

fn safe_relative_path(relative_path: &str) -> Result<PathBuf, String> {
    if relative_path.is_empty()
        || relative_path.contains('\\')
        || relative_path
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(format!(
            "{DIST_WEB_READY_PATH} contains an unsafe file path: {relative_path}"
        ));
    }
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "{DIST_WEB_READY_PATH} contains an unsafe file path: {relative_path}"
        ));
    }
    Ok(relative.to_path_buf())
}

fn manifest_file_path(relative_path: &str) -> Result<PathBuf, String> {
    Ok(Path::new(DIST_WEB_ROOT_PATH).join(safe_relative_path(relative_path)?))
}

fn marker_digest<'a>(
    files: &'a serde_json::Map<String, serde_json::Value>,
    relative_path: &str,
) -> Result<&'a str, String> {
    let digest = files
        .get(relative_path)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!("{DIST_WEB_READY_PATH} has no digest for published file {relative_path}")
        })?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "{DIST_WEB_READY_PATH} has an invalid digest for published file {relative_path}"
        ));
    }
    Ok(digest)
}

fn index_referenced_asset_paths(index: &str) -> Result<BTreeSet<String>, String> {
    let mut assets = BTreeSet::new();
    for quote in ['"', '\''] {
        let prefix = format!("{quote}/ui/");
        let mut remaining = index;
        while let Some(position) = remaining.find(&prefix) {
            let value = &remaining[position + prefix.len()..];
            let end = value.find(quote).ok_or_else(|| {
                format!("{DIST_WEB_INDEX_PATH} contains an unterminated /ui/ asset reference")
            })?;
            let relative_path = value[..end].split(['?', '#']).next().unwrap_or_default();
            if !relative_path.is_empty() {
                safe_relative_path(relative_path)?;
                assets.insert(relative_path.to_string());
            }
            remaining = &value[end + quote.len_utf8()..];
        }
    }
    Ok(assets)
}

fn validate_prebuilt_web_ui_locked() -> Result<ValidatedWebUi, String> {
    if !Path::new(DIST_WEB_INDEX_PATH).is_file() {
        return Err(format!("{DIST_WEB_INDEX_PATH} is missing"));
    }
    if !Path::new(DIST_WEB_READY_PATH).is_file() {
        return Err(format!("{DIST_WEB_READY_PATH} is missing"));
    }

    let marker_text = fs::read_to_string(DIST_WEB_READY_PATH)
        .map_err(|error| format!("failed to read {DIST_WEB_READY_PATH}: {error}"))?;
    let marker: serde_json::Value = serde_json::from_str(&marker_text)
        .map_err(|error| format!("failed to parse {DIST_WEB_READY_PATH}: {error}"))?;
    let schema_version = marker
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("{DIST_WEB_READY_PATH} has no schemaVersion"))?;
    if schema_version != READY_MARKER_SCHEMA_VERSION {
        return Err(format!(
            "{DIST_WEB_READY_PATH} has unsupported schemaVersion {schema_version}"
        ));
    }
    let expected_index_digest = marker
        .get("indexSha256")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{DIST_WEB_READY_PATH} has no indexSha256"))?;
    let files = marker
        .get("files")
        .and_then(serde_json::Value::as_object)
        .filter(|files| !files.is_empty())
        .ok_or_else(|| format!("{DIST_WEB_READY_PATH} has no files manifest"))?;
    let index = fs::read(DIST_WEB_INDEX_PATH)
        .map_err(|error| format!("failed to read {DIST_WEB_INDEX_PATH}: {error}"))?;
    let actual_index_digest = sha256_hex(&index);
    if actual_index_digest != expected_index_digest {
        return Err(format!(
            "{DIST_WEB_READY_PATH} describes index digest {expected_index_digest}, but {DIST_WEB_INDEX_PATH} has {actual_index_digest}"
        ));
    }
    let manifest_index_digest = marker_digest(files, "index.html")?;
    if manifest_index_digest != expected_index_digest {
        return Err(format!(
            "{DIST_WEB_READY_PATH} indexSha256 does not match files[index.html]"
        ));
    }

    let mut validated_files = Vec::with_capacity(files.len());
    for relative_path in files.keys() {
        let expected_file_digest = marker_digest(files, relative_path)?;
        let published_path = manifest_file_path(relative_path)?;
        let contents = fs::read(&published_path).map_err(|error| {
            format!(
                "failed to read published web console file {}: {error}",
                published_path.display()
            )
        })?;
        let actual_file_digest = sha256_hex(contents);
        if actual_file_digest != expected_file_digest {
            return Err(format!(
                "{DIST_WEB_READY_PATH} describes {relative_path} digest {expected_file_digest}, but {} has {actual_file_digest}",
                published_path.display()
            ));
        }
        validated_files.push(ValidatedWebFile {
            relative_path: relative_path.clone(),
            source_path: published_path,
            expected_digest: expected_file_digest.to_string(),
        });
    }

    let index_text = String::from_utf8(index)
        .map_err(|error| format!("{DIST_WEB_INDEX_PATH} is not valid UTF-8: {error}"))?;
    for relative_path in index_referenced_asset_paths(&index_text)? {
        marker_digest(files, &relative_path)?;
    }

    assert_ready_marker_unchanged(&marker_text)?;
    Ok(ValidatedWebUi {
        marker_text,
        files: validated_files,
    })
}

fn assert_ready_marker_unchanged(expected_marker: &str) -> Result<(), String> {
    let marker_after = fs::read_to_string(DIST_WEB_READY_PATH)
        .map_err(|error| format!("failed to reread {DIST_WEB_READY_PATH}: {error}"))?;
    if marker_after != expected_marker {
        return Err(format!(
            "{DIST_WEB_READY_PATH} marker changed during validation and snapshotting"
        ));
    }
    Ok(())
}

fn with_publish_validation_lock<T>(
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let validation_lock = PublishValidationLock::acquire()?;
    let operation_result = operation();
    let release_result = validation_lock.release();
    match (operation_result, release_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(operation_error), Err(release_error)) => Err(format!(
            "{operation_error}; additionally failed to release validation lock: {release_error}"
        )),
    }
}

#[cfg(test)]
fn validate_prebuilt_web_ui() -> Result<(), String> {
    with_publish_validation_lock(|| validate_prebuilt_web_ui_locked().map(|_| ()))
}

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

fn copy_validated_files_to_directory<F>(
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

fn publish_web_ui_snapshot(snapshot_root: &Path, published: &ValidatedWebUi) -> Result<(), String> {
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

fn snapshot_prebuilt_web_ui(snapshot_root: &Path) -> Result<(), String> {
    with_publish_validation_lock(|| {
        // Keep validation, copy, copied-digest verification, and the final marker
        // stability check inside the same publisher lock critical section.
        let published = validate_prebuilt_web_ui_locked()?;
        publish_web_ui_snapshot(snapshot_root, &published)?;
        assert_ready_marker_unchanged(&published.marker_text)
    })
}

fn wait_for_prebuilt_web_ui_snapshot(snapshot_root: &Path) -> Result<(), String> {
    let mut last_error = String::new();
    for attempt in 0..PREBUILT_WEB_UI_WAIT_ATTEMPTS {
        match snapshot_prebuilt_web_ui(snapshot_root) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
        if attempt + 1 < PREBUILT_WEB_UI_WAIT_ATTEMPTS {
            thread::sleep(PREBUILT_WEB_UI_WAIT_INTERVAL);
        }
    }
    Err(last_error)
}

fn prepare_web_ui_snapshot<F>(
    prebuilt_web_ui_enabled: bool,
    snapshot_root: &Path,
    build_web_ui: F,
) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    if !prebuilt_web_ui_enabled {
        build_web_ui()?;
    }
    wait_for_prebuilt_web_ui_snapshot(snapshot_root)
}

fn write_embedded_ui_source(out_dir: &Path, snapshot_root: &Path) -> Result<PathBuf, String> {
    let snapshot_path = snapshot_root.to_str().ok_or_else(|| {
        format!(
            "web UI snapshot path is not valid UTF-8: {}",
            snapshot_root.display()
        )
    })?;
    fs::create_dir_all(out_dir).map_err(|error| {
        format!(
            "failed to create generated RustEmbed source directory {}: {error}",
            out_dir.display()
        )
    })?;
    let generated_path = out_dir.join(GENERATED_EMBED_SOURCE_NAME);
    let source = format!(
        "#[derive(rust_embed::RustEmbed)]\n#[folder = {snapshot_path:?}]\nstruct EmbeddedUi;\nconst _: &str = env!(\"{WEB_UI_SNAPSHOT_ENV}\");\n"
    );
    fs::write(&generated_path, source).map_err(|error| {
        format!(
            "failed to write generated RustEmbed source {}: {error}",
            generated_path.display()
        )
    })?;
    Ok(generated_path)
}

fn snapshot_rustc_env_directive(snapshot_root: &Path) -> Result<String, String> {
    let snapshot_path = snapshot_root.to_str().ok_or_else(|| {
        format!(
            "web UI snapshot path is not valid UTF-8: {}",
            snapshot_root.display()
        )
    })?;
    if snapshot_path.contains('\r') || snapshot_path.contains('\n') {
        return Err("web UI snapshot path contains a newline".to_string());
    }
    Ok(format!(
        "cargo:rustc-env={WEB_UI_SNAPSHOT_ENV}={snapshot_path}"
    ))
}

fn main() {
    println!("cargo:rerun-if-env-changed={PREBUILT_WEB_UI_ENV}");
    for path in [
        "apps/desktop/package.json",
        "apps/desktop/package-lock.json",
        "apps/desktop/rsbuild.config.ts",
        "apps/desktop/tsconfig.json",
        "apps/desktop/public",
        "apps/desktop/src",
        "apps/desktop/tools",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    let prebuilt_web_ui_enabled = env::var(PREBUILT_WEB_UI_ENV)
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "True"))
        .unwrap_or(false);
    if prebuilt_web_ui_enabled {
        println!("cargo:rerun-if-changed={DIST_WEB_READY_PATH}");
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must provide OUT_DIR"));
    let snapshot_root = out_dir.join(WEB_UI_SNAPSHOT_DIR_NAME);
    let preparation_result =
        prepare_web_ui_snapshot(prebuilt_web_ui_enabled, &snapshot_root, || {
            let npm_program = if cfg!(windows) { "npm.cmd" } else { "npm" };
            let status = Command::new(npm_program)
                .args(["run", "build:web", "--prefix", "apps/desktop"])
                .env("GATEWAY_WEB_PRUNE_LIVE", "1")
                .status()
                .map_err(|error| {
                    format!("failed to launch npm for Gateway web console build: {error}")
                })?;
            if status.success() {
                Ok(())
            } else {
                Err(format!(
                    "Gateway web console build failed with status: {status}"
                ))
            }
        });
    if let Err(error) = preparation_result {
        if prebuilt_web_ui_enabled {
            panic!(
                "prebuilt web console assets requested via {PREBUILT_WEB_UI_ENV}, but the published assets could not be snapshotted: {error}"
            );
        }
        panic!("Gateway web console build or immutable snapshot failed: {error}");
    }

    if prebuilt_web_ui_enabled {
        println!(
            "cargo:warning=prebuilt web console assets requested via {PREBUILT_WEB_UI_ENV}; verified ready marker and created immutable build snapshot"
        );
    }
    write_embedded_ui_source(&out_dir, &snapshot_root)
        .unwrap_or_else(|error| panic!("failed to configure embedded web UI snapshot: {error}"));
    println!(
        "{}",
        snapshot_rustc_env_directive(&snapshot_root)
            .unwrap_or_else(|error| panic!("failed to export web UI snapshot path: {error}"))
    );
}
