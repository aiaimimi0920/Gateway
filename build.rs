#[path = "build_support/embed_source.rs"]
mod embed_source;
#[path = "build_support/manifest.rs"]
mod manifest;
#[path = "build_support/publish_lock.rs"]
mod publish_lock;
#[path = "build_support/snapshot.rs"]
mod snapshot;
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
