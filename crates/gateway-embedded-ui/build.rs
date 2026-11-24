//! Isolated build script for embedded Web UI assets so frontend rebuilds never recompile neuro_gateway.
#[path = "../../build_support/embed_source.rs"]
mod embed_source;
#[path = "../../build_support/manifest.rs"]
mod manifest;
#[path = "../../build_support/publish_lock.rs"]
mod publish_lock;
#[path = "../../build_support/snapshot.rs"]
mod snapshot;

use embed_source::{snapshot_rustc_env_directive, write_embedded_ui_source};
use manifest::{assert_ready_marker_unchanged, safe_relative_path, validate_prebuilt_web_ui_locked};
use publish_lock::{unix_time_nanos, with_publish_validation_lock};
use sha2::{Digest, Sha256};
use snapshot::publish_web_ui_snapshot;
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

fn snapshot_prebuilt_web_ui(snapshot_root: &Path) -> Result<(), String> {
    with_publish_validation_lock(|| {
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

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo_root = fs::canonicalize(manifest_dir.join("../..")).expect("resolve Gateway root");
    env::set_current_dir(&repo_root).expect("enter Gateway root for web UI snapshot");

    println!("cargo:rerun-if-env-changed={PREBUILT_WEB_UI_ENV}");
    let prebuilt_enabled = env::var(PREBUILT_WEB_UI_ENV)
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "True"))
        .unwrap_or(false);

    if prebuilt_enabled {
        // When prebuilt assets are supplied, depend only on the published ready marker
        // so unrelated TypeScript edits do not re-trigger snapshot copies unnecessarily.
        println!("cargo:rerun-if-changed={}", repo_root.join(DIST_WEB_READY_PATH).display());
    } else {
        for rel in [
            "apps/desktop/package.json",
            "apps/desktop/package-lock.json",
            "apps/desktop/rsbuild.config.ts",
            "apps/desktop/tsconfig.json",
            "apps/desktop/public",
            "apps/desktop/src",
            "apps/desktop/tools",
        ] {
            println!("cargo:rerun-if-changed={}", repo_root.join(rel).display());
        }
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let snapshot_root = out_dir.join(WEB_UI_SNAPSHOT_DIR_NAME);
    if !prebuilt_enabled {
        let npm_program = if cfg!(windows) { "npm.cmd" } else { "npm" };
        let status = Command::new(npm_program)
            .args(["run", "build:web", "--prefix", "apps/desktop"])
            .env("GATEWAY_WEB_PRUNE_LIVE", "1")
            .status()
            .unwrap_or_else(|err| panic!("failed to launch npm for Gateway web console build: {err}"));
        assert!(status.success(), "Gateway web console build failed with status: {status}");
    }
    if let Err(error) = wait_for_prebuilt_web_ui_snapshot(&snapshot_root) {
        if prebuilt_enabled {
            panic!("prebuilt web console assets requested via {PREBUILT_WEB_UI_ENV}, but snapshot failed: {error}");
        }
        panic!("Gateway web console snapshot failed: {error}");
    }
    write_embedded_ui_source(&out_dir, &snapshot_root)
        .unwrap_or_else(|err| panic!("failed to configure embedded web UI snapshot: {err}"));
    println!(
        "{}",
        snapshot_rustc_env_directive(&snapshot_root)
            .unwrap_or_else(|err| panic!("failed to export web UI snapshot path: {err}"))
    );
}
