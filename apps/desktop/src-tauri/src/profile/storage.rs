use super::GatewayProfile;
use crate::paths::gateway_profile_dir;
use std::fs;
use std::path::{Path, PathBuf};
fn sanitize_profile_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("profile name cannot be empty".to_string());
    }

    let mut sanitized = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }
    Ok(sanitized)
}

pub(super) fn profile_path(name: &str) -> Result<PathBuf, String> {
    let sanitized = sanitize_profile_name(name)?;
    Ok(gateway_profile_dir()?.join(format!("{sanitized}.json")))
}

pub(super) fn read_profile_from_path(path: &Path) -> Result<GatewayProfile, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read profile {}: {error}", path.display()))?;
    serde_json::from_str::<GatewayProfile>(&text)
        .map_err(|error| format!("failed to parse profile {}: {error}", path.display()))
}
