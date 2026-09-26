use super::inventory::effective_credential_id;
use super::runtime::now_rfc3339;
use crate::config::Config;
use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml};
use serde::Serialize;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;
const CREDENTIAL_ARCHIVE_DIRECTORY: &str = "_archive";

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

pub fn provider_credential_storage_path(config: &Config, provider_id: &str) -> Option<PathBuf> {
    provider_credential_storage_root_path(config)
        .map(|root| root.join(safe_archive_path_segment(provider_id)))
}

pub fn provider_credential_archive_path(config: &Config, provider_id: &str) -> Option<PathBuf> {
    provider_credential_storage_root_path(config).map(|root| {
        root.join(CREDENTIAL_ARCHIVE_DIRECTORY)
            .join(safe_archive_path_segment(provider_id))
    })
}

pub fn archived_provider_credential_count(config: &Config, provider_id: &str) -> usize {
    let Some(directory) = provider_credential_archive_path(config, provider_id) else {
        return 0;
    };
    archived_credential_count_in_directory(&directory)
}

pub(super) fn archived_credential_count_in_directory(directory: &Path) -> usize {
    let Ok(entries) = fs::read_dir(directory) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| {
            matches!(entry.file_type(), Ok(file_type) if file_type.is_file() && !file_type.is_symlink())
                && entry.path().extension().and_then(|value| value.to_str()) == Some("json")
        })
        .count()
}

pub fn purge_provider_credential_archive(
    config: &Config,
    provider_id: &str,
) -> anyhow::Result<usize> {
    let directory = provider_credential_archive_path(config, provider_id)
        .ok_or_else(|| anyhow::anyhow!("credential storage path is unavailable"))?;
    purge_credential_archive_directory(&directory)
}

pub(super) fn purge_credential_archive_directory(directory: &Path) -> anyhow::Result<usize> {
    let metadata = match fs::symlink_metadata(&directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        anyhow::bail!("credential archive path is not a regular directory");
    }

    let mut purged_count = 0usize;
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() || !file_type.is_file() {
            continue;
        }
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        fs::remove_file(entry.path())?;
        purged_count += 1;
    }
    if fs::read_dir(&directory)?.next().is_none() {
        fs::remove_dir(&directory)?;
    }
    Ok(purged_count)
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

pub(super) fn archive_pruned_credentials(
    config: &Config,
    provider: &ProviderConfigYaml,
    prune_ids: &HashSet<String>,
) -> anyhow::Result<usize> {
    if prune_ids.is_empty() || provider.credential_permanent_delete_enabled {
        return Ok(0);
    }
    let directory = provider_credential_archive_path(config, &provider.id)
        .ok_or_else(|| anyhow::anyhow!("credential archive path is unavailable"))?;
    archive_pruned_credentials_in_directory(provider, prune_ids, &directory)
}

pub(super) fn archive_pruned_credentials_in_directory(
    provider: &ProviderConfigYaml,
    prune_ids: &HashSet<String>,
    directory: &Path,
) -> anyhow::Result<usize> {
    fs::create_dir_all(&directory)?;
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
