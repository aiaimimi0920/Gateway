//! Generated RustEmbed source and compiler snapshot-path directive.

use super::*;

pub(super) fn write_embedded_ui_source(
    out_dir: &Path,
    snapshot_root: &Path,
) -> Result<PathBuf, String> {
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

pub(super) fn snapshot_rustc_env_directive(snapshot_root: &Path) -> Result<String, String> {
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
