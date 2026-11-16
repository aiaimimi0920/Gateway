//! Persist verified downloads across desktop restarts, scoped to the chosen source and channel.
use super::model::{self, Preferences, Release};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
struct Pending {
    release: Release,
    package: PathBuf,
    channel: String,
    source: String,
}
pub fn save(
    root: &Path,
    release: &Release,
    package: &Path,
    prefs: &Preferences,
) -> Result<(), String> {
    let value = Pending {
        release: release.clone(),
        package: package.to_owned(),
        channel: prefs.channel.clone(),
        source: prefs.download_source.clone(),
    };
    let temporary = root.join("updates/pending.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec(&value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temporary, root.join("updates/pending.json")).map_err(|e| e.to_string())
}
pub fn load(root: &Path, prefs: &Preferences) -> Result<Option<(Release, PathBuf)>, String> {
    let path = root.join("updates/pending.json");
    if !path.exists() {
        return Ok(None);
    }
    let value: Pending = model::read_json(&path)?;
    if value.channel != prefs.channel || value.source != prefs.download_source {
        return Ok(None);
    }
    if model::version(&model::installed().0).is_some_and(|current| {
        model::version(&value.release.version).is_none_or(|next| next <= current)
    }) {
        return Ok(None);
    }
    let package = value
        .package
        .canonicalize()
        .map_err(|_| "Downloaded update package is missing")?;
    if !package.starts_with(
        root.join("updates")
            .canonicalize()
            .map_err(|e| e.to_string())?,
    ) || !package
        .parent()
        .is_some_and(|parent| parent.join("prepared").is_file())
    {
        return Err("Invalid downloaded update location".into());
    }
    Ok(Some((value.release, package)))
}
