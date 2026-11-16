//! Stage a new immutable package. Never replace live binaries or write into an existing package.
use super::{
    model::{Preferences, Release},
    releases,
};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn powershell(script: &Path, request: &Path) -> Command {
    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .arg("-RequestPath")
        .arg(request);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}
pub fn stage(root: &Path, prefs: &Preferences, release: &Release) -> Result<PathBuf, String> {
    if !cfg!(target_os = "windows") {
        return Err("Only Windows desktop update packages are published".into());
    }
    let hash = releases::checksum(release)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let directory = root.join("updates").join(format!("download-{stamp}"));
    gateway_local_data::restore::create_private_directory(&directory).map_err(|e| e.to_string())?;
    let url = if prefs.download_source.is_empty() {
        release.asset.clone()
    } else {
        format!(
            "{}/{}",
            prefs.download_source.trim_end_matches('/'),
            release.asset
        )
    };
    let response = releases::client(Duration::from_secs(900))?
        .get(url)
        .send()
        .map_err(|_| "Update download failed")?;
    if !response.status().is_success() {
        return Err(format!(
            "Update download HTTP {}",
            response.status().as_u16()
        ));
    }
    let archive = directory.join("release.zip");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&archive)
        .map_err(|e| e.to_string())?;
    let length = std::io::copy(&mut response.take(2 * 1024 * 1024 * 1024 + 1), &mut file)
        .map_err(|_| "Cannot write update download")?;
    file.flush()
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    if length > 2 * 1024 * 1024 * 1024 {
        return Err("Update download exceeds 2 GiB".into());
    }
    let package = directory.join("package");
    let request = directory.join("prepare.json");
    fs::write(&request, serde_json::to_vec(&serde_json::json!({"archive":archive,"directory":package,"sha256":hash,"version":release.version})).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let script = directory.join("prepare.ps1");
    fs::write(&script, include_str!("prepare.ps1")).map_err(|e| e.to_string())?;
    let output = powershell(&script, &request)
        .output()
        .map_err(|_| "Cannot run update package verification")?;
    if !output.status.success() || !directory.join("prepared").is_file() {
        return Err(
            "Update package verification failed; existing installation was not modified".into(),
        );
    }
    Ok(package)
}
