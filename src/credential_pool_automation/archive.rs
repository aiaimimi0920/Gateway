use super::inventory::effective_credential_id;
use super::runtime::now_rfc3339;
use super::storage_paths::{
    provider_archive_path, provider_storage_path, reject_linked_components,
};
use crate::config::Config;
use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArchivedProviderCredentialRecord<'a> {
    schema_version: u8,
    provider_id: &'a str,
    credential_id: &'a str,
    archived_at: String,
    reason: &'a str,
    credential: &'a ProviderCredentialYaml,
}

pub fn provider_credential_storage_root_path(config: &Config) -> Option<PathBuf> {
    config
        .provider_credential_folder_sync_root_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn provider_credential_storage_path(
    config: &Config,
    provider: &ProviderConfigYaml,
) -> Option<PathBuf> {
    provider_storage_path(config, provider)
}

pub fn provider_credential_archive_path(
    config: &Config,
    provider: &ProviderConfigYaml,
) -> Option<PathBuf> {
    provider_archive_path(config, provider)
}

pub fn archived_provider_credential_count(config: &Config, provider: &ProviderConfigYaml) -> usize {
    let Some(directory) = provider_credential_archive_path(config, provider) else {
        return 0;
    };
    if reject_linked_components(&directory).is_err() {
        return 0;
    }
    archived_credential_count_in_directory(&directory, &provider.id)
}

pub(crate) fn checked_local_archive_count(
    config: &Config,
    provider: &ProviderConfigYaml,
) -> anyhow::Result<usize> {
    let Some(directory) = provider_credential_archive_path(config, provider) else {
        return Ok(0);
    };
    checked_archive_count_in_directory(&directory, &provider.id)
}

pub(super) fn checked_archive_count_in_directory(
    directory: &Path,
    provider_id: &str,
) -> anyhow::Result<usize> {
    reject_linked_components(&directory)?;
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let mut count = 0;
    for (scanned, entry) in entries.enumerate() {
        if scanned >= 1024 {
            anyhow::bail!("Archive directory scan limit exceeded");
        }
        if checked_provider_archive(&entry?.path(), provider_id)? {
            count += 1;
        }
    }
    Ok(count)
}

pub(super) fn archived_credential_count_in_directory(directory: &Path, provider_id: &str) -> usize {
    let Ok(entries) = fs::read_dir(directory) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| is_provider_archive(&entry.path(), provider_id))
        .count()
}

#[cfg(test)]
pub fn purge_provider_credential_archive(
    config: &Config,
    provider: &ProviderConfigYaml,
) -> anyhow::Result<usize> {
    purge_archive_with_guard(config, provider, &|| true)
}

#[cfg(test)]
pub(crate) fn purge_archive_with_guard(
    config: &Config,
    provider: &ProviderConfigYaml,
    current_revision: &(dyn Fn() -> bool + Send + Sync),
) -> anyhow::Result<usize> {
    if let Some(path) = &provider.credential_archive_path {
        super::storage_paths::validate_storage_path(path).map_err(anyhow::Error::msg)?;
    }
    let directory = provider_credential_archive_path(config, provider)
        .ok_or_else(|| anyhow::anyhow!("credential storage path is unavailable"))?;
    purge_directory_with_guard(&directory, &provider.id, current_revision)
}

#[cfg(test)]
pub(super) fn purge_credential_archive_directory(
    directory: &Path,
    provider_id: &str,
) -> anyhow::Result<usize> {
    purge_directory_with_guard(directory, provider_id, &|| true)
}

#[cfg(test)]
fn purge_directory_with_guard(
    directory: &Path,
    provider_id: &str,
    current_revision: &(dyn Fn() -> bool + Send + Sync),
) -> anyhow::Result<usize> {
    reject_linked_components(directory)?;
    let metadata = match fs::symlink_metadata(&directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        anyhow::bail!("credential archive path is not a regular directory");
    }

    let mut purged_count = 0usize;
    for (scanned, entry) in fs::read_dir(&directory)?.enumerate() {
        if scanned >= 1024 {
            anyhow::bail!(
                "Archive purge scan limit exceeded; completed deletions are not rolled back"
            );
        }
        let entry = entry?;
        if !checked_provider_archive(&entry.path(), provider_id)? {
            continue;
        }
        if !current_revision() {
            anyhow::bail!(
                "Route revision changed during purge; completed deletions are not rolled back"
            );
        }
        fs::remove_file(entry.path())?;
        purged_count += 1;
    }
    // The configured directory may be a shared/mounted root; retain it even when empty.
    Ok(purged_count)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveIdentity {
    schema_version: u8,
    provider_id: String,
    credential_id: String,
    archived_at: String,
    reason: String,
    credential: ProviderCredentialYaml,
}

fn is_provider_archive(path: &Path, provider_id: &str) -> bool {
    checked_provider_archive(path, provider_id).unwrap_or(false)
}

fn checked_provider_archive(path: &Path, provider_id: &str) -> anyhow::Result<bool> {
    const MAX_RECORD_BYTES: u64 = 4 * 1024 * 1024;
    if path.extension().and_then(|value| value.to_str()) != Some("json") {
        return Ok(false);
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Ok(false);
    }
    if metadata.len() > MAX_RECORD_BYTES {
        anyhow::bail!("Archive record exceeds the read limit");
    }
    let file = fs::File::open(path)?;
    checked_archive_reader(file, provider_id)
}

pub(super) fn checked_archive_reader(file: impl Read, provider_id: &str) -> anyhow::Result<bool> {
    const MAX_RECORD_BYTES: u64 = 4 * 1024 * 1024;
    let mut bytes = Vec::new();
    file.take(MAX_RECORD_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        anyhow::bail!("Archive record exceeds the read limit");
    }
    let Ok(record) = serde_json::from_slice::<ArchiveIdentity>(&bytes) else {
        return Ok(false);
    };
    Ok(record.schema_version == 1
        && record.provider_id == provider_id
        && !record.credential_id.trim().is_empty()
        && time::OffsetDateTime::parse(
            &record.archived_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_ok()
        && record.reason == "permanent_driver_rejection"
        && record
            .credential
            .id
            .as_deref()
            .is_none_or(|id| id == record.credential_id))
}

pub(super) fn safe_archive_path_segment(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if normalized.is_empty() || normalized == "." || normalized == ".." {
        "provider".to_string()
    } else {
        normalized
    }
}

pub(crate) fn archive_pruned_credentials(
    config: &Config,
    provider: &ProviderConfigYaml,
    prune_ids: &HashSet<String>,
) -> anyhow::Result<usize> {
    if prune_ids.is_empty() || provider.credential_permanent_delete_enabled {
        return Ok(0);
    }
    if let Some(path) = &provider.credential_archive_path {
        super::storage_paths::validate_storage_path(path).map_err(anyhow::Error::msg)?;
    }
    let directory = provider_credential_archive_path(config, provider)
        .ok_or_else(|| anyhow::anyhow!("credential archive path is unavailable"))?;
    archive_pruned_credentials_in_directory(provider, prune_ids, &directory)
}

pub(super) fn archive_pruned_credentials_in_directory(
    provider: &ProviderConfigYaml,
    prune_ids: &HashSet<String>,
    directory: &Path,
) -> anyhow::Result<usize> {
    reject_linked_components(directory)?;
    fs::create_dir_all(&directory)?;
    reject_linked_components(directory)?;
    let metadata = fs::symlink_metadata(&directory)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        anyhow::bail!("credential archive path is not a regular directory");
    }

    let archived_at = now_rfc3339();
    let archive_stamp = archived_at.replace(':', "-").replace('.', "-");
    let mut archived_count = 0usize;
    for (index, credential) in provider.credentials.iter().enumerate() {
        let credential_id = effective_credential_id(&provider.id, index, credential);
        if !prune_ids.contains(&credential_id) {
            continue;
        }
        let record = ArchivedProviderCredentialRecord {
            schema_version: 1,
            provider_id: &provider.id,
            credential_id: &credential_id,
            archived_at: archived_at.clone(),
            reason: "permanent_driver_rejection",
            credential,
        };
        let bytes = serde_json::to_vec_pretty(&record)?;
        let base_name = format!(
            "{}-{}-{}",
            archive_stamp,
            safe_archive_path_segment(&credential_id),
            Uuid::new_v4()
        );
        let temporary_path = directory.join(format!(".{base_name}.tmp"));
        let archive_path = directory.join(format!("{base_name}.json"));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary_path)?;
        if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error.into());
        }
        drop(file);
        if let Err(error) = fs::rename(&temporary_path, &archive_path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error.into());
        }
        archived_count += 1;
    }
    Ok(archived_count)
}
