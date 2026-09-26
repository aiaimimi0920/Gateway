//! Published web UI manifest, asset path and digest validation.

use super::*;

pub(super) fn safe_relative_path(relative_path: &str) -> Result<PathBuf, String> {
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

pub(super) fn validate_prebuilt_web_ui_locked() -> Result<ValidatedWebUi, String> {
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

pub(super) fn assert_ready_marker_unchanged(expected_marker: &str) -> Result<(), String> {
    let marker_after = fs::read_to_string(DIST_WEB_READY_PATH)
        .map_err(|error| format!("failed to reread {DIST_WEB_READY_PATH}: {error}"))?;
    if marker_after != expected_marker {
        return Err(format!(
            "{DIST_WEB_READY_PATH} marker changed during validation and snapshotting"
        ));
    }
    Ok(())
}
