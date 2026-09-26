use aws_sdk_s3::primitives::ByteStream;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::time::timeout;
use uuid::Uuid;

use super::body_limit::{ensure_object_size, limit_for_object_key, read_bounded, read_local_file};
use super::local_paths::{
    ensure_local_path_confined, ensure_local_path_if_root_exists, local_object_path,
    local_operation_guard,
};
use super::{network_timeout, GatewayObjectStorage, ObjectStorageDriver};
use crate::error::GatewayError;

impl GatewayObjectStorage {
    pub async fn put_json(&self, object_key: &str, payload: &Value) -> Result<(), GatewayError> {
        let bytes = serde_json::to_vec_pretty(payload).map_err(|error| {
            GatewayError::server_error(format!("serialize object payload: {error}"))
        })?;
        self.put_bytes(object_key, bytes, "application/json").await
    }

    pub async fn read_json(&self, object_key: &str) -> Result<Value, GatewayError> {
        let bytes = self.read_bytes(object_key).await?;
        serde_json::from_slice(&bytes).map_err(|error| {
            GatewayError::server_error(format!("parse object payload json: {error}"))
        })
    }

    pub async fn put_bytes(
        &self,
        object_key: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<(), GatewayError> {
        ensure_object_size(bytes.len() as u64, limit_for_object_key(object_key))?;
        match &self.driver {
            ObjectStorageDriver::Local { root } => {
                let _operation_guard = local_operation_guard(root).await;
                let path = local_object_path(root, object_key)?;
                fs::create_dir_all(root).await.map_err(|error| {
                    GatewayError::service_unavailable(format!(
                        "create object storage root: {error}"
                    ))
                })?;
                ensure_local_path_confined(root, &path).await?;
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).await.map_err(|error| {
                        GatewayError::service_unavailable(format!(
                            "create object storage directory: {error}"
                        ))
                    })?;
                    ensure_local_path_confined(root, &path).await?;
                }
                let temporary_path = local_temporary_path(&path)?;
                ensure_local_path_confined(root, &temporary_path).await?;
                if let Err(error) = fs::write(&temporary_path, bytes).await {
                    let _ = fs::remove_file(&temporary_path).await;
                    return Err(GatewayError::service_unavailable(format!(
                        "write object payload: {error}"
                    )));
                }
                if let Err(error) = fs::rename(&temporary_path, &path).await {
                    let _ = fs::remove_file(&temporary_path).await;
                    return Err(GatewayError::service_unavailable(format!(
                        "replace object payload: {error}"
                    )));
                }
            }
            ObjectStorageDriver::S3Compatible {
                client,
                bucket,
                deadline,
            } => {
                timeout(
                    *deadline,
                    client
                        .put_object()
                        .bucket(bucket)
                        .key(object_key)
                        .content_type(content_type)
                        .body(ByteStream::from(bytes))
                        .send(),
                )
                .await
                .map_err(|_| network_timeout("put", *deadline))?
                .map_err(|error| {
                    GatewayError::service_unavailable(format!("put object: {error}"))
                })?;
            }
        }
        Ok(())
    }

    pub async fn read_bytes(&self, object_key: &str) -> Result<Vec<u8>, GatewayError> {
        let limit = limit_for_object_key(object_key);
        match &self.driver {
            ObjectStorageDriver::Local { root } => {
                let _operation_guard = local_operation_guard(root).await;
                let path = local_object_path(root, object_key)?;
                ensure_local_path_if_root_exists(root, &path).await?;
                read_local_file(&path, limit).await
            }
            ObjectStorageDriver::S3Compatible {
                client,
                bucket,
                deadline,
            } => timeout(*deadline, async {
                let response = client
                    .get_object()
                    .bucket(bucket)
                    .key(object_key)
                    .send()
                    .await
                    .map_err(|error| {
                        GatewayError::service_unavailable(format!("get object: {error}"))
                    })?;
                let length = response
                    .content_length
                    .and_then(|length| u64::try_from(length).ok());
                read_bounded(response.body.into_async_read(), length, limit).await
            })
            .await
            .map_err(|_| network_timeout("get", *deadline))?,
        }
    }

    pub async fn delete_object(&self, object_key: &str) -> Result<(), GatewayError> {
        if object_key.trim().is_empty() {
            return Ok(());
        }
        match &self.driver {
            ObjectStorageDriver::Local { root } => {
                let _operation_guard = local_operation_guard(root).await;
                let path = local_object_path(root, object_key)?;
                ensure_local_path_if_root_exists(root, &path).await?;
                match fs::remove_file(path).await {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(GatewayError::service_unavailable(format!(
                            "delete object payload: {error}"
                        )))
                    }
                }
            }
            ObjectStorageDriver::S3Compatible {
                client,
                bucket,
                deadline,
            } => {
                timeout(
                    *deadline,
                    client.delete_object().bucket(bucket).key(object_key).send(),
                )
                .await
                .map_err(|_| network_timeout("delete", *deadline))?
                .map_err(|error| {
                    GatewayError::service_unavailable(format!("delete object: {error}"))
                })?;
            }
        }
        Ok(())
    }
}

fn local_temporary_path(path: &Path) -> Result<PathBuf, GatewayError> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| GatewayError::server_error("build local object temporary path"))?;
    Ok(path.with_file_name(format!(".{file_name}.tmp-{}", Uuid::new_v4())))
}
