//! Refill-specific folder ingestion stays inside one provider and the route CAS.
//! It deliberately does not invoke the unrelated global PostgreSQL folder importer.
use super::*;
use crate::credential_pool_automation::storage_paths::{
    reject_linked_components, validate_storage_path,
};
use crate::{error::GatewayError, state::AppState};
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_DELIVERY_BYTES: usize = 4 * 1024 * 1024;

pub(super) async fn collect(
    state: &Arc<AppState>,
    task: &CredentialRefillTaskRecord,
    relative_paths: Vec<String>,
) -> Result<Vec<ProviderCredentialYaml>, GatewayError> {
    validate_folder_paths(&relative_paths)?;
    let snapshot = state.route_config.snapshot();
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == task.provider_id)
        .ok_or_else(|| GatewayError::not_found("Refill provider does not exist"))?;
    if let Some(path) = &provider.credential_storage_path {
        validate_storage_path(path).map_err(GatewayError::bad_request)?;
    }
    let root = crate::credential_pool_automation::provider_credential_storage_path(
        &state.config,
        provider,
    )
    .ok_or_else(|| {
        GatewayError::bad_request("Provider credential storage directory is not configured")
    })?;
    let requested = task.requested_count.min(MAX_REQUESTED_COUNT);
    // Own only bounded file work across cancellation; no route mutation happens in this worker.
    tokio::task::spawn_blocking(move || read_credentials(&root, &relative_paths, requested))
        .await
        .map_err(|_| GatewayError::server_error("Refill folder reader failed"))?
}

fn read_credentials(
    root: &Path,
    relative_paths: &[String],
    requested: usize,
) -> Result<Vec<ProviderCredentialYaml>, GatewayError> {
    reject_linked_components(root).map_err(|_| invalid_path())?;
    let root = fs::canonicalize(root).map_err(|_| invalid_path())?;
    let paths = if relative_paths.is_empty() {
        let mut paths = Vec::new();
        for (scanned, entry) in fs::read_dir(&root).map_err(|_| invalid_path())?.enumerate() {
            if scanned >= 1_024 {
                return Err(GatewayError::bad_request(
                    "Refill directory scan limit reached; select explicit relativePaths",
                ));
            }
            let entry = entry.map_err(|_| invalid_path())?;
            if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            paths.push(entry.file_name().to_string_lossy().into_owned());
            if paths.len() > requested.min(MAX_FOLDER_PATHS) {
                return Err(GatewayError::bad_request(
                    "Refill directory has too many JSON files; select explicit relativePaths",
                ));
            }
        }
        paths.sort();
        paths
    } else {
        relative_paths.to_vec()
    };
    if paths.len() > requested || paths.len() > MAX_FOLDER_PATHS {
        return Err(GatewayError::bad_request(
            "Refill file count exceeds the requested batch",
        ));
    }
    let mut total_bytes = 0usize;
    let mut credentials = Vec::with_capacity(paths.len());
    for relative in paths {
        let path = relative_file(&root, &relative)?;
        reject_linked_components(&path).map_err(|_| invalid_path())?;
        let metadata = fs::symlink_metadata(&path).map_err(|_| invalid_path())?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > MAX_FILE_BYTES
        {
            return Err(invalid_path());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.nlink() > 1 {
                return Err(invalid_path());
            }
        }
        let mut bytes = Vec::new();
        fs::File::open(&path)
            .map_err(|_| invalid_path())?
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid_path())?;
        total_bytes = total_bytes.saturating_add(bytes.len());
        if bytes.len() as u64 > MAX_FILE_BYTES || total_bytes > MAX_DELIVERY_BYTES {
            return Err(GatewayError::bad_request(
                "Refill JSON exceeds the bounded delivery size",
            ));
        }
        let credential =
            serde_json::from_slice::<ProviderCredentialYaml>(&bytes).map_err(|_| {
                GatewayError::bad_request(
                    "Refill file must contain one ProviderCredentialYaml JSON object",
                )
            })?;
        credentials.push(credential);
    }
    Ok(credentials)
}

fn relative_file(root: &Path, value: &str) -> Result<PathBuf, GatewayError> {
    let path = Path::new(value);
    if value.contains(['\\', ':'])
        || path.extension().and_then(|s| s.to_str()) != Some("json")
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(invalid_path());
    }
    Ok(root.join(path))
}
fn invalid_path() -> GatewayError {
    GatewayError::bad_request(
        "Refill path must identify a regular JSON file inside this provider's storage directory",
    )
    .with_code("credential_refill_folder_path_invalid")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_paths_cannot_escape_or_name_other_protocols() {
        let root = Path::new("/fixture");
        for value in [
            "../other.json",
            "/other.json",
            "https://host/a.json",
            "a\\b.json",
            "a.txt",
        ] {
            assert!(relative_file(root, value).is_err(), "{value}");
        }
        assert_eq!(
            relative_file(root, "nested/a.json").unwrap(),
            root.join("nested/a.json")
        );
    }
    #[test]
    fn reads_only_provider_json_and_enforces_file_and_batch_bounds() {
        let root = std::env::temp_dir().join(format!("refill-folder-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("a.json"), r#"{"id":"a","api_key":"fixture"}"#).unwrap();
        assert_eq!(
            read_credentials(&root, &["a.json".into()], 1)
                .unwrap()
                .len(),
            1
        );
        assert!(read_credentials(&root, &["a.json".into()], 0).is_err());
        let file = fs::File::create(root.join("large.json")).unwrap();
        file.set_len(MAX_FILE_BYTES + 1).unwrap();
        assert!(read_credentials(&root, &["large.json".into()], 1).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("a.json"), root.join("link.json")).unwrap();
            assert!(read_credentials(&root, &["link.json".into()], 1).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
