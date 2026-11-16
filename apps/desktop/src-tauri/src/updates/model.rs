//! Desktop-owned update preferences and release identity; never inferred from a remote backend.
use serde::{Deserialize, Serialize};
use std::{fs, io::Read, path::Path};

pub const PROJECT: &str = "https://github.com/aiaimimi0920/Gateway";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Preferences {
    pub show_button: bool,
    pub automatic: bool,
    pub interval_hours: u32,
    pub channel: String,
    pub auto_download: bool,
    pub auto_restart: bool,
    pub download_source: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            show_button: true,
            automatic: false,
            interval_hours: 24,
            channel: "stable".into(),
            auto_download: false,
            auto_restart: false,
            download_source: String::new(),
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
        if ![1, 6, 24].contains(&self.interval_hours)
            || !["stable", "all"].contains(&self.channel.as_str())
        {
            return Err("Invalid update interval or channel".into());
        }
        if self.auto_restart && (!self.auto_download || !self.automatic) {
            return Err("Automatic restart requires automatic checks and downloads".into());
        }
        if self.auto_download && !self.automatic {
            return Err("Automatic download requires automatic checks".into());
        }
        if !self.download_source.is_empty() {
            let url = reqwest::Url::parse(&self.download_source)
                .map_err(|_| "Invalid download source")?;
            if self.download_source.len() > 2048
                || url.scheme() != "https"
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err("Download source must be an HTTPS prefix without credentials or query parameters".into());
            }
        }
        Ok(())
    }
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("Update metadata exceeds size limit".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Invalid update metadata".into())
}
pub fn preferences(root: &Path) -> Result<Preferences, String> {
    let path = root.join("update-settings.json");
    let prefs: Preferences = if path.exists() {
        read_json(&path)?
    } else {
        Preferences::default()
    };
    prefs.validate()?;
    Ok(prefs)
}
pub fn save(root: &Path, prefs: &Preferences) -> Result<(), String> {
    prefs.validate()?;
    let temporary = root.join("update-settings.pending.json");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(prefs).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temporary, root.join("update-settings.json")).map_err(|e| e.to_string())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub page: String,
    pub asset: String,
    pub checksum: String,
}
pub fn version(value: &str) -> Option<[u64; 3]> {
    let value = value
        .strip_prefix('V')
        .or_else(|| value.strip_prefix('v'))
        .unwrap_or(value);
    let numbers = value
        .split('.')
        .map(|part| {
            (!part.is_empty() && part.len() <= 10 && part.bytes().all(|b| b.is_ascii_digit()))
                .then(|| part.parse().ok())
                .flatten()
        })
        .collect::<Option<Vec<u64>>>()?;
    numbers.try_into().ok()
}
pub fn installed() -> (String, bool) {
    let manifest = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("manifest.json")))
        .and_then(|path| read_json::<serde_json::Value>(&path).ok());
    if let Some(manifest) = manifest {
        let value = manifest["versionId"]
            .as_str()
            .unwrap_or("unknown")
            .to_owned();
        let formal = manifest["buildKind"] == "release"
            && manifest["edition"] == "desktop"
            && manifest["target"] == "windows-x64"
            && version(&value).is_some();
        return (value, formal);
    }
    (
        format!("{} (development)", env!("CARGO_PKG_VERSION")),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_order_and_preference_dependencies_are_explicit() {
        assert!(version("V1.10.0") > version("v1.9.9"));
        for value in ["1.2", "../1.2.3", "1.2.3-beta", "1.2.3.4", "+1.2.3"] {
            assert!(version(value).is_none());
        }
        let mut prefs = Preferences::default();
        assert!(prefs.validate().is_ok());
        prefs.auto_restart = true;
        assert!(prefs.validate().is_err());
        prefs.auto_download = true;
        prefs.automatic = true;
        assert!(prefs.validate().is_ok());
        prefs.download_source = "https://user:secret@example.com".into();
        assert!(prefs.validate().is_err());
    }
}
