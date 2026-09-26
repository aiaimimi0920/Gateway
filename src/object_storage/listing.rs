use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::time::timeout;

use super::local_paths::{local_object_path, local_operation_guard};
use super::{network_timeout, GatewayObjectStorage, ObjectStorageDriver};
use crate::error::GatewayError;

impl GatewayObjectStorage {
    pub async fn list_objects(&self, prefix: &str) -> Result<Vec<String>, GatewayError> {
        match &self.driver {
            ObjectStorageDriver::Local { root } => {
                let _operation_guard = local_operation_guard(root).await;
                let start = if prefix.is_empty() {
                    root.to_path_buf()
                } else {
                    local_object_path(root, prefix)?
                };
                match tokio::fs::metadata(&start).await {
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        return Ok(Vec::new());
                    }
                    Err(error) => {
                        return Err(GatewayError::service_unavailable(format!(
                            "inspect local object storage listing root: {error}"
                        )));
                    }
                }
                let canonical_root = tokio::fs::canonicalize(root).await.map_err(|error| {
                    GatewayError::service_unavailable(format!(
                        "resolve local object storage root: {error}"
                    ))
                })?;
                let mut results = Vec::new();
                let mut visited_directories = HashSet::new();
                collect_local_object_keys(
                    root,
                    &canonical_root,
                    &start,
                    &mut visited_directories,
                    &mut results,
                )
                .await
                .map_err(|error| {
                    GatewayError::service_unavailable(format!("list object payloads: {error}"))
                })?;
                results.sort();
                Ok(results)
            }
            ObjectStorageDriver::S3Compatible {
                client,
                bucket,
                deadline,
            } => {
                let mut continuation_token = None;
                let mut results = Vec::new();
                loop {
                    let mut request = client
                        .list_objects_v2()
                        .bucket(bucket)
                        .prefix(prefix.to_string());
                    if let Some(token) = continuation_token.as_deref() {
                        request = request.continuation_token(token);
                    }
                    let response = timeout(*deadline, request.send())
                        .await
                        .map_err(|_| network_timeout("list", *deadline))?
                        .map_err(|error| {
                            GatewayError::service_unavailable(format!("list objects: {error}"))
                        })?;
                    if let Some(contents) = response.contents {
                        results.extend(contents.into_iter().filter_map(|item| item.key));
                    }
                    if response.is_truncated.unwrap_or(false) {
                        let next_token = response
                            .next_continuation_token
                            .filter(|token| !token.is_empty());
                        // A stalled cursor would otherwise replay pages indefinitely.
                        if next_token.is_none() || next_token == continuation_token {
                            return Err(GatewayError::service_unavailable(
                                "S3 object listing returned a non-advancing continuation token",
                            )
                            .with_code("object_storage_pagination_stalled"));
                        }
                        continuation_token = next_token;
                    } else {
                        break;
                    }
                }
                results.sort();
                Ok(results)
            }
        }
    }
}

async fn collect_local_object_keys(
    root: &Path,
    canonical_root: &Path,
    current: &Path,
    visited_directories: &mut HashSet<PathBuf>,
    results: &mut Vec<String>,
) -> Result<(), std::io::Error> {
    let mut pending = vec![current.to_path_buf()];
    while let Some(current) = pending.pop() {
        let canonical_current = tokio::fs::canonicalize(&current).await?;
        if !canonical_current.starts_with(canonical_root) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "object storage path escapes configured root",
            ));
        }
        let metadata = tokio::fs::metadata(&current).await?;
        if metadata.is_file() {
            if let Ok(relative) = current.strip_prefix(root) {
                let key = relative
                    .components()
                    .map(|component| component.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                results.push(key);
            }
            continue;
        }
        if !metadata.is_dir() || !visited_directories.insert(canonical_current) {
            continue;
        }
        let mut entries = tokio::fs::read_dir(&current).await?;
        while let Some(entry) = entries.next_entry().await? {
            pending.push(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
