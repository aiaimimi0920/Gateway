//! Provider-local filesystem locations; remote object-storage URLs need a real protocol.
use crate::{config::Config, routing::config::ProviderConfigYaml};
use std::path::{Component, Path, PathBuf};

pub fn validate_storage_path(value: &str) -> Result<(), &'static str> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(());
    }
    if value.contains("://") {
        return Err("尚未配置云存储协议，请使用本地/已挂载云盘目录");
    }
    let path = Path::new(value);
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        || value.chars().any(char::is_control)
    {
        return Err("凭证存储目录必须为绝对本地路径，且不能包含父目录跳转");
    }
    Ok(())
}

pub fn provider_storage_path(config: &Config, provider: &ProviderConfigYaml) -> Option<PathBuf> {
    configured_path(provider.credential_storage_path.as_deref()).or_else(|| {
        super::archive::provider_credential_storage_root_path(config)
            .map(|root| root.join(super::archive::safe_archive_path_segment(&provider.id)))
    })
}

pub fn provider_archive_path(config: &Config, provider: &ProviderConfigYaml) -> Option<PathBuf> {
    configured_path(provider.credential_archive_path.as_deref()).or_else(|| {
        super::archive::provider_credential_storage_root_path(config).map(|root| {
            root.join("_archive")
                .join(super::archive::safe_archive_path_segment(&provider.id))
        })
    })
}

fn configured_path(value: Option<&str>) -> Option<PathBuf> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Reject linked path components before any credential file I/O. This is a static
/// containment check; an administrator can still replace directories concurrently.
pub(crate) fn reject_linked_components(path: &Path) -> anyhow::Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            anyhow::bail!("parent path traversal is not allowed");
        }
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    anyhow::bail!("credential path contains a symbolic link");
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if metadata.file_attributes() & 0x400 != 0 {
                        anyhow::bail!("credential path contains a reparse point");
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_remote_protocols_and_relative_traversal() {
        assert!(validate_storage_path("https://example.invalid/archive").is_err());
        assert!(validate_storage_path("s3://bucket/key").is_err());
        assert!(validate_storage_path("../credentials").is_err());
        assert!(validate_storage_path("/tmp/../credentials").is_err());
        assert!(validate_storage_path("").is_ok());
        assert!(validate_storage_path(std::env::temp_dir().to_str().unwrap()).is_ok());
    }
}
