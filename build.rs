use std::{env, path::Path, process::Command};

const PREBUILT_WEB_UI_ENV: &str = "GATEWAY_PREBUILT_WEB_UI";
const DIST_WEB_INDEX_PATH: &str = "apps/desktop/dist/web/index.html";

fn main() {
    println!("cargo:rerun-if-env-changed=GATEWAY_PREBUILT_WEB_UI");
    for path in [
        "apps/desktop/package.json",
        "apps/desktop/package-lock.json",
        "apps/desktop/rsbuild.config.ts",
        "apps/desktop/tsconfig.json",
        "apps/desktop/src",
        "apps/desktop/dist/web",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    let prebuilt_web_ui_enabled = env::var(PREBUILT_WEB_UI_ENV)
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "True"))
        .unwrap_or(false);
    if prebuilt_web_ui_enabled {
        if Path::new(DIST_WEB_INDEX_PATH).is_file() {
            println!(
                "cargo:warning=prebuilt web console assets requested via {PREBUILT_WEB_UI_ENV}; skipping npm build"
            );
            return;
        }
        panic!(
            "prebuilt web console assets requested via {PREBUILT_WEB_UI_ENV}, but {DIST_WEB_INDEX_PATH} is missing"
        );
    }

    let npm_program = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let status = Command::new(npm_program)
        .args(["run", "build:web", "--prefix", "apps/desktop"])
        .status()
        .expect("failed to launch npm for Gateway web console build");
    if !status.success() {
        panic!("Gateway web console build failed with status: {status}");
    }
}
