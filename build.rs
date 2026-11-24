#[allow(dead_code)]
#[path = "build_support/embed_source.rs"]
mod embed_source;
#[path = "build_support/manifest.rs"]
mod manifest;
#[path = "build_support/publish_lock.rs"]
mod publish_lock;
#[path = "build_support/snapshot.rs"]
mod snapshot;
#[allow(unused_imports)]
use embed_source::{snapshot_rustc_env_directive, write_embedded_ui_source};
use manifest::{
    assert_ready_marker_unchanged, safe_relative_path, validate_prebuilt_web_ui_locked,
};
#[cfg(test)]
use publish_lock::{process_is_alive, recover_stale_publish_lock_with};
use publish_lock::{unix_time_nanos, with_publish_validation_lock};
#[cfg(test)]
use snapshot::copy_validated_files_to_directory;
use snapshot::publish_web_ui_snapshot;

use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Component, Path, PathBuf},
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
/// How long an owner-less lock directory has to sit untouched before it counts as
/// abandoned. A publisher writes its owner file immediately after creating the
/// directory, so any gap longer than this means the publisher never got there.
const PUBLISH_LOCK_OWNERLESS_GRACE: Duration = Duration::from_secs(10);

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

fn sha256_hex(contents: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(contents.as_ref()))
}

#[cfg(test)]
fn validate_prebuilt_web_ui() -> Result<(), String> {
    with_publish_validation_lock(|| validate_prebuilt_web_ui_locked().map(|_| ()))
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

#[allow(dead_code)]
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

fn main() {
    println!("cargo:rerun-if-changed=build_support/windows.rc");
    println!("cargo:rerun-if-changed=apps/desktop/src-tauri/icons/icon.ico");
    // Only the headless EXE owns this resource; Web UI embedding lives in crates/gateway-embedded-ui.
    #[cfg(not(test))]
    embed_resource::compile_for(
        "build_support/windows.rc",
        ["gateway"],
        embed_resource::NONE,
    )
    .manifest_required()
    .expect("failed to embed the shared Gateway Windows icon");
}
