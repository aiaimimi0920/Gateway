//! Public GitHub release transport. No Gateway authentication header is ever attached.
use super::model::{version, Preferences, Release, PROJECT};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{io::Read, time::Duration};

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}
#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

pub fn client(timeout: Duration) -> Result<Client, String> {
    Client::builder()
        .user_agent("Neuro-Gateway-Updater")
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || attempt.url().scheme() != "https" {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| "Cannot initialize update connection".into())
}
fn bounded_text(url: &str, limit: usize) -> Result<String, String> {
    let response = client(Duration::from_secs(25))?
        .get(url)
        .send()
        .map_err(|_| "Cannot connect to GitHub releases")?;
    if !response.status().is_success() {
        return Err(format!("GitHub HTTP {}", response.status().as_u16()));
    }
    let mut bytes = Vec::new();
    response
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read update metadata")?;
    if bytes.len() > limit {
        return Err("Update metadata exceeds size limit".into());
    }
    String::from_utf8(bytes).map_err(|_| "Invalid update metadata encoding".into())
}
pub fn check(prefs: &Preferences, current: &str) -> Result<Option<Release>, String> {
    let body = bounded_text(
        "https://api.github.com/repos/aiaimimi0920/Gateway/releases?per_page=30",
        2 * 1024 * 1024,
    )?;
    select(&body, prefs, current)
}
fn select(body: &str, prefs: &Preferences, current: &str) -> Result<Option<Release>, String> {
    let releases: Vec<GithubRelease> =
        serde_json::from_str(body).map_err(|_| "Invalid GitHub release response")?;
    if releases.len() > 30 {
        return Err("Too many release records".into());
    }
    let mut matches = Vec::new();
    for release in releases {
        let Some(number) = version(&release.tag_name) else {
            continue;
        };
        if release.draft
            || (release.prerelease && prefs.channel == "stable")
            || version(current).is_some_and(|old| number <= old)
        {
            continue;
        }
        let prefix = format!("{PROJECT}/releases/download/{}/", release.tag_name);
        let filename = format!("Gateway-{}-windows-x64.zip", release.tag_name);
        let asset = release.assets.iter().find(|asset| {
            asset.name == filename && asset.browser_download_url == format!("{prefix}{filename}")
        });
        let checksum = release.assets.iter().find(|asset| {
            asset.name == format!("{filename}.sha256")
                && asset.browser_download_url == format!("{prefix}{filename}.sha256")
        });
        if !release
            .html_url
            .starts_with(&format!("{PROJECT}/releases/tag/"))
        {
            continue;
        }
        if let (Some(asset), Some(checksum)) = (asset, checksum) {
            matches.push((
                number,
                Release {
                    version: release.tag_name,
                    notes: release.body.unwrap_or_default(),
                    page: release.html_url,
                    asset: asset.browser_download_url.clone(),
                    checksum: checksum.browser_download_url.clone(),
                },
            ));
        }
    }
    matches.sort_by_key(|(number, _)| *number);
    Ok(matches.pop().map(|(_, release)| release))
}
pub fn checksum(release: &Release) -> Result<String, String> {
    let body = bounded_text(&release.checksum, 4096)?;
    let fields: Vec<_> = body.split_whitespace().collect();
    let filename = format!("Gateway-{}-windows-x64.zip", release.version);
    if fields.len() != 2
        || fields[1].trim_start_matches('*') != filename
        || fields[0].len() != 64
        || !fields[0].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("Invalid official release checksum".into());
    }
    Ok(fields[0].to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn never_accepts_external_assets_or_downgrades() {
        let mut release = serde_json::json!({"tag_name":"V1.2.3","html_url":format!("{PROJECT}/releases/tag/V1.2.3"),"body":"notes","draft":false,"prerelease":false,"assets":[
            {"name":"Gateway-V1.2.3-windows-x64.zip","browser_download_url":format!("{PROJECT}/releases/download/V1.2.3/Gateway-V1.2.3-windows-x64.zip")},
            {"name":"Gateway-V1.2.3-windows-x64.zip.sha256","browser_download_url":format!("{PROJECT}/releases/download/V1.2.3/Gateway-V1.2.3-windows-x64.zip.sha256")}]});
        let encode = |value: &serde_json::Value| serde_json::json!([value]).to_string();
        assert!(select(&encode(&release), &Preferences::default(), "V1.0.0")
            .unwrap()
            .is_some());
        assert!(select(&encode(&release), &Preferences::default(), "V2.0.0")
            .unwrap()
            .is_none());
        release["assets"][0]["browser_download_url"] = "https://evil.example/installer.zip".into();
        assert!(select(&encode(&release), &Preferences::default(), "V1.0.0")
            .unwrap()
            .is_none());
    }
}
