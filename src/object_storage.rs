use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use serde_json::Value;
use tokio::fs;
use tokio::time::timeout;
use uuid::Uuid;

use crate::error::GatewayError;

enum ObjectStorageDriver {
    Local { root: PathBuf },
    S3Compatible { client: Client, bucket: String },
}

pub struct GatewayObjectStorage {
    driver: ObjectStorageDriver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectStorageProbeOutcome {
    pub ready: bool,
    pub timed_out: bool,
}

impl GatewayObjectStorage {
    pub(crate) fn from_env() -> Result<Self, GatewayError> {
        let driver = std::env::var("AI_GATEWAY_OBJECT_STORAGE_DRIVER")
            .ok()
            .or_else(|| std::env::var("OBJECT_STORAGE_DRIVER").ok())
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "local".to_string());

        if driver == "s3-compatible" {
            let bucket =
                required_env(&["AI_GATEWAY_OBJECT_STORAGE_BUCKET", "OBJECT_STORAGE_BUCKET"])?;
            let endpoint = required_env(&[
                "AI_GATEWAY_OBJECT_STORAGE_ENDPOINT",
                "OBJECT_STORAGE_ENDPOINT",
            ])?;
            let access_key_id = required_env(&[
                "AI_GATEWAY_OBJECT_STORAGE_ACCESS_KEY_ID",
                "OBJECT_STORAGE_ACCESS_KEY_ID",
            ])?;
            let secret_access_key = required_env(&[
                "AI_GATEWAY_OBJECT_STORAGE_SECRET_ACCESS_KEY",
                "OBJECT_STORAGE_SECRET_ACCESS_KEY",
            ])?;
            let region =
                optional_env(&["AI_GATEWAY_OBJECT_STORAGE_REGION", "OBJECT_STORAGE_REGION"])
                    .unwrap_or_else(|| "auto".to_string());
            let force_path_style = parse_bool_env(
                optional_env(&[
                    "AI_GATEWAY_OBJECT_STORAGE_FORCE_PATH_STYLE",
                    "OBJECT_STORAGE_FORCE_PATH_STYLE",
                ])
                .as_deref(),
                false,
            );

            let credentials = Credentials::new(access_key_id, secret_access_key, None, None, "env");
            let mut builder = aws_sdk_s3::config::Builder::new()
                .region(Region::new(region))
                .credentials_provider(credentials)
                .force_path_style(force_path_style)
                .behavior_version_latest();
            builder = builder.endpoint_url(endpoint);
            let client = Client::from_conf(builder.build());

            return Ok(Self {
                driver: ObjectStorageDriver::S3Compatible { client, bucket },
            });
        }

        let root = optional_env(&[
            "AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR",
            "CREDENTIAL_OBJECT_STORAGE_LOCAL_DIR",
            "OBJECT_STORAGE_LOCAL_DIR",
        ])
        .unwrap_or_else(|| ".runtime/ai-gateway-objects".to_string());

        Ok(Self {
            driver: ObjectStorageDriver::Local {
                root: PathBuf::from(root),
            },
        })
    }

    pub async fn probe_readiness(&self, deadline: Duration) -> ObjectStorageProbeOutcome {
        match &self.driver {
            ObjectStorageDriver::Local { root } => {
                probe_local_object_storage(root.as_path(), deadline).await
            }
            ObjectStorageDriver::S3Compatible { client, bucket } => {
                match timeout(deadline, client.head_bucket().bucket(bucket).send()).await {
                    Ok(Ok(_)) => ObjectStorageProbeOutcome {
                        ready: true,
                        timed_out: false,
                    },
                    Ok(Err(_)) => ObjectStorageProbeOutcome {
                        ready: false,
                        timed_out: false,
                    },
                    Err(_) => ObjectStorageProbeOutcome {
                        ready: false,
                        timed_out: true,
                    },
                }
            }
        }
    }

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
        match &self.driver {
            ObjectStorageDriver::Local { root } => {
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
                fs::write(path, bytes).await.map_err(|error| {
                    GatewayError::service_unavailable(format!("write object payload: {error}"))
                })?;
            }
            ObjectStorageDriver::S3Compatible { client, bucket } => {
                client
                    .put_object()
                    .bucket(bucket)
                    .key(object_key)
                    .content_type(content_type)
                    .body(ByteStream::from(bytes))
                    .send()
                    .await
                    .map_err(|error| {
                        GatewayError::service_unavailable(format!("put object: {error}"))
                    })?;
            }
        }
        Ok(())
    }

    pub async fn read_bytes(&self, object_key: &str) -> Result<Vec<u8>, GatewayError> {
        match &self.driver {
            ObjectStorageDriver::Local { root } => {
                let path = local_object_path(root, object_key)?;
                ensure_local_path_if_root_exists(root, &path).await?;
                fs::read(path).await.map_err(|error| {
                    GatewayError::service_unavailable(format!("read object payload: {error}"))
                })
            }
            ObjectStorageDriver::S3Compatible { client, bucket } => {
                let response = client
                    .get_object()
                    .bucket(bucket)
                    .key(object_key)
                    .send()
                    .await
                    .map_err(|error| {
                        GatewayError::service_unavailable(format!("get object: {error}"))
                    })?;
                let bytes = response.body.collect().await.map_err(|error| {
                    GatewayError::service_unavailable(format!("collect object bytes: {error}"))
                })?;
                Ok(bytes.into_bytes().to_vec())
            }
        }
    }

    pub async fn delete_object(&self, object_key: &str) -> Result<(), GatewayError> {
        if object_key.trim().is_empty() {
            return Ok(());
        }
        match &self.driver {
            ObjectStorageDriver::Local { root } => {
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
            ObjectStorageDriver::S3Compatible { client, bucket } => {
                client
                    .delete_object()
                    .bucket(bucket)
                    .key(object_key)
                    .send()
                    .await
                    .map_err(|error| {
                        GatewayError::service_unavailable(format!("delete object: {error}"))
                    })?;
            }
        }
        Ok(())
    }

    pub async fn list_objects(&self, prefix: &str) -> Result<Vec<String>, GatewayError> {
        match &self.driver {
            ObjectStorageDriver::Local { root } => {
                let start = if prefix.is_empty() {
                    root.to_path_buf()
                } else {
                    local_object_path(root, prefix)?
                };
                if !start.exists() {
                    return Ok(Vec::new());
                }
                let canonical_root = std::fs::canonicalize(root).map_err(|error| {
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
                .map_err(|error| {
                    GatewayError::service_unavailable(format!("list object payloads: {error}"))
                })?;
                results.sort();
                Ok(results)
            }
            ObjectStorageDriver::S3Compatible { client, bucket } => {
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
                    let response = request.send().await.map_err(|error| {
                        GatewayError::service_unavailable(format!("list objects: {error}"))
                    })?;
                    if let Some(contents) = response.contents {
                        results.extend(contents.into_iter().filter_map(|item| item.key));
                    }
                    if response.is_truncated.unwrap_or(false) {
                        continuation_token = response.next_continuation_token;
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

pub fn gateway_object_storage() -> Result<&'static GatewayObjectStorage, GatewayError> {
    static STORAGE: OnceLock<Result<GatewayObjectStorage, String>> = OnceLock::new();
    match STORAGE.get_or_init(|| GatewayObjectStorage::from_env().map_err(|e| e.message)) {
        Ok(storage) => Ok(storage),
        Err(message) => Err(GatewayError::service_unavailable(message.clone())),
    }
}

pub fn build_gateway_provider_account_object_key(provider_account_id: &str) -> String {
    format!("ai-gateway/provider-account/{provider_account_id}.json")
}

pub fn build_gateway_provider_credential_object_key(provider_credential_id: &str) -> String {
    format!("ai-gateway/provider-credential/{provider_credential_id}.json")
}

pub fn build_gateway_analysis_export_prefix(export_id: &str) -> String {
    format!("ai-gateway/analysis-exports/{export_id}")
}

pub fn build_gateway_analysis_export_object_key(export_id: &str, file_name: &str) -> String {
    format!(
        "{}/{}",
        build_gateway_analysis_export_prefix(export_id),
        file_name
    )
}

pub fn build_gateway_analysis_export_manifest_object_key(export_id: &str) -> String {
    build_gateway_analysis_export_object_key(export_id, "manifest.json")
}

pub fn build_gateway_analysis_export_dataset_object_key(export_id: &str) -> String {
    build_gateway_analysis_export_object_key(export_id, "dataset.jsonl")
}

pub fn build_gateway_conversation_archive_object_key(archive_id: &str, file_name: &str) -> String {
    format!("ai-gateway/conversation-archives/{archive_id}/{file_name}")
}

pub fn build_gateway_conversation_archive_export_dataset_object_key(export_id: &str) -> String {
    format!("ai-gateway/conversation-archive-exports/{export_id}/dataset.jsonl")
}

pub fn build_gateway_conversation_dataset_export_object_key(
    dataset_id: &str,
    file_name: &str,
) -> String {
    format!("ai-gateway/conversation-dataset-exports/{dataset_id}/{file_name}")
}

pub fn choose_provider_payload_storage_mode(payload: &Value) -> &'static str {
    let serialized = serde_json::to_vec(payload).unwrap_or_default();
    if serialized.len() > 4096 {
        return "r2";
    }

    let has_large_array = payload
        .as_object()
        .map(|map| {
            map.values()
                .any(|value| matches!(value, Value::Array(values) if values.len() > 50))
        })
        .unwrap_or(false);
    if has_large_array {
        return "r2";
    }

    "inline"
}

fn local_object_path(root: &Path, object_key: &str) -> Result<PathBuf, GatewayError> {
    validate_local_object_key(object_key)?;
    Ok(object_key
        .split('/')
        .fold(root.to_path_buf(), |path, segment| path.join(segment)))
}

fn validate_local_object_key(object_key: &str) -> Result<(), GatewayError> {
    if object_key.is_empty()
        || object_key.contains('\0')
        || object_key.contains('\\')
        || object_key.starts_with('/')
        || object_key.starts_with("//")
        || object_key
            .as_bytes()
            .get(1)
            .is_some_and(|byte| *byte == b':')
    {
        return Err(
            GatewayError::bad_request("invalid local object storage key")
                .with_code("object_storage_key_invalid"),
        );
    }

    let path = Path::new(object_key);
    if path.is_absolute() || path.has_root() {
        return Err(
            GatewayError::bad_request("invalid local object storage key")
                .with_code("object_storage_key_invalid"),
        );
    }

    if object_key.split('/').any(is_invalid_local_object_segment) {
        return Err(
            GatewayError::bad_request("invalid local object storage key")
                .with_code("object_storage_key_invalid"),
        );
    }
    Ok(())
}

fn is_invalid_local_object_segment(segment: &str) -> bool {
    if segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.contains(':')
        || segment.ends_with('.')
        || segment.ends_with(' ')
        || Path::new(segment)
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return true;
    }

    let device_stem = segment
        .split_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(segment);
    let upper_device_stem = device_stem.to_ascii_uppercase();
    matches!(upper_device_stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper_device_stem
            .strip_prefix("COM")
            .is_some_and(is_reserved_device_number)
        || upper_device_stem
            .strip_prefix("LPT")
            .is_some_and(is_reserved_device_number)
}

fn is_reserved_device_number(suffix: &str) -> bool {
    matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
}

async fn ensure_local_path_if_root_exists(
    root: &Path,
    candidate: &Path,
) -> Result<(), GatewayError> {
    match fs::symlink_metadata(root).await {
        Ok(_) => ensure_local_path_confined(root, candidate).await,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(GatewayError::service_unavailable(format!(
            "inspect local object storage root: {error}"
        ))),
    }
}

async fn ensure_local_path_confined(root: &Path, candidate: &Path) -> Result<(), GatewayError> {
    let canonical_root = fs::canonicalize(root).await.map_err(|error| {
        GatewayError::service_unavailable(format!("resolve local object storage root: {error}"))
    })?;
    let mut existing = candidate.to_path_buf();
    loop {
        match fs::symlink_metadata(&existing).await {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !existing.pop() {
                    return Err(GatewayError::bad_request(
                        "local object storage path escapes configured root",
                    )
                    .with_code("object_storage_path_escape"));
                }
            }
            Err(error) => {
                return Err(GatewayError::service_unavailable(format!(
                    "inspect local object storage path: {error}"
                )))
            }
        }
    }

    let canonical_existing = fs::canonicalize(&existing).await.map_err(|error| {
        GatewayError::service_unavailable(format!("resolve local object storage path: {error}"))
    })?;
    if !canonical_existing.starts_with(&canonical_root) {
        return Err(
            GatewayError::bad_request("local object storage path escapes configured root")
                .with_code("object_storage_path_escape"),
        );
    }
    Ok(())
}

fn collect_local_object_keys(
    root: &Path,
    canonical_root: &Path,
    current: &Path,
    visited_directories: &mut HashSet<PathBuf>,
    results: &mut Vec<String>,
) -> Result<(), std::io::Error> {
    let canonical_current = std::fs::canonicalize(current)?;
    if !canonical_current.starts_with(canonical_root) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "object storage path escapes configured root",
        ));
    }
    if current.is_file() {
        if let Ok(relative) = current.strip_prefix(root) {
            let key = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            results.push(key);
        }
        return Ok(());
    }
    if !visited_directories.insert(canonical_current) {
        return Ok(());
    }
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        collect_local_object_keys(
            root,
            canonical_root,
            &entry.path(),
            visited_directories,
            results,
        )?;
    }
    Ok(())
}

fn optional_env(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn required_env(names: &[&str]) -> Result<String, GatewayError> {
    optional_env(names).ok_or_else(|| {
        GatewayError::service_unavailable(format!("缺少对象存储配置: {}", names.join(" / ")))
    })
}

fn parse_bool_env(value: Option<&str>, fallback: bool) -> bool {
    let Some(value) = value else {
        return fallback;
    };
    match value.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use uuid::Uuid;

    #[test]
    fn choose_storage_mode_uses_inline_for_small_payloads() {
        let payload = serde_json::json!({
            "adapter": "openai_compatible",
            "apiKey": "small",
        });
        assert_eq!(choose_provider_payload_storage_mode(&payload), "inline");
    }

    #[test]
    fn choose_storage_mode_uses_object_for_large_arrays() {
        let payload = serde_json::json!({
            "adapter": "openai_compatible",
            "apiKeys": vec!["k"; 51],
        });
        assert_eq!(choose_provider_payload_storage_mode(&payload), "r2");
    }

    #[tokio::test]
    async fn local_readiness_probe_round_trips_and_cleans_probe_file() {
        let root =
            std::env::temp_dir().join(format!("neuro-gateway-object-readiness-{}", Uuid::new_v4()));
        let storage = GatewayObjectStorage {
            driver: ObjectStorageDriver::Local { root: root.clone() },
        };

        let outcome = storage.probe_readiness(Duration::from_secs(1)).await;

        assert!(outcome.ready);
        assert!(!outcome.timed_out);
        let remaining = std::fs::read_dir(&root)
            .expect("read probe root")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect probe root entries");
        assert!(remaining.is_empty(), "readiness probe must clean its file");
        std::fs::remove_dir_all(root).expect("remove probe root");
    }

    #[tokio::test]
    async fn local_readiness_probe_rejects_non_directory_root() {
        let root = std::env::temp_dir().join(format!(
            "neuro-gateway-object-readiness-file-{}",
            Uuid::new_v4()
        ));
        std::fs::write(&root, b"not-a-directory").expect("write invalid probe root");
        let storage = GatewayObjectStorage {
            driver: ObjectStorageDriver::Local { root: root.clone() },
        };

        let outcome = storage.probe_readiness(Duration::from_secs(1)).await;

        assert!(!outcome.ready);
        assert!(!outcome.timed_out);
        std::fs::remove_file(root).expect("remove invalid probe root");
    }

    #[tokio::test]
    async fn local_object_storage_rejects_unsafe_object_keys() {
        let base =
            std::env::temp_dir().join(format!("neuro-gateway-object-key-{}", Uuid::new_v4()));
        let root = base.join("root");
        let storage = GatewayObjectStorage {
            driver: ObjectStorageDriver::Local { root: root.clone() },
        };

        for object_key in [
            "../escape.json",
            "/rooted.json",
            "folder\\escape.json",
            "folder/./escape.json",
            "folder/\0escape.json",
            "folder/file:stream.json",
            "folder/file.",
            "folder/file ",
            "CON",
            "con.txt",
            "folder/PRN.log",
            "AUX",
            "NUL",
            "COM1",
            "LPT9.txt",
        ] {
            let error = storage
                .put_bytes(
                    object_key,
                    b"must-not-be-written".to_vec(),
                    "application/json",
                )
                .await
                .expect_err("unsafe local object key must be rejected");
            assert_eq!(
                error.code.as_deref(),
                Some("object_storage_key_invalid"),
                "unexpected error for object key {object_key:?}: {error}"
            );
        }

        assert!(!base.join("escape.json").exists());
        if base.exists() {
            std::fs::remove_dir_all(base).expect("remove object key test root");
        }
    }

    #[tokio::test]
    async fn local_list_objects_accepts_an_empty_prefix() {
        let root =
            std::env::temp_dir().join(format!("neuro-gateway-object-list-{}", Uuid::new_v4()));
        let storage = GatewayObjectStorage {
            driver: ObjectStorageDriver::Local { root: root.clone() },
        };
        storage
            .put_bytes(
                "nested/object.json",
                b"payload".to_vec(),
                "application/json",
            )
            .await
            .expect("write object for empty-prefix listing");

        let objects = storage
            .list_objects("")
            .await
            .expect("empty prefix should list local objects");

        assert_eq!(objects, vec!["nested/object.json"]);
        std::fs::remove_dir_all(root).expect("remove empty-prefix listing root");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn local_object_storage_rejects_junction_escape() {
        let base =
            std::env::temp_dir().join(format!("neuro-gateway-object-junction-{}", Uuid::new_v4()));
        let root = base.join("root");
        let outside = base.join("outside");
        let junction = root.join("linked");
        std::fs::create_dir_all(&root).expect("create local object root");
        std::fs::create_dir_all(&outside).expect("create junction target");
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .output()
            .expect("create test junction");
        assert!(
            output.status.success(),
            "failed to create test junction: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let storage = GatewayObjectStorage {
            driver: ObjectStorageDriver::Local { root: root.clone() },
        };

        let error = storage
            .put_bytes(
                "linked/escape.json",
                b"must-not-escape".to_vec(),
                "application/json",
            )
            .await
            .expect_err("junction escape must be rejected");

        assert_eq!(error.code.as_deref(), Some("object_storage_path_escape"));
        assert!(!outside.join("escape.json").exists());
        std::fs::remove_dir_all(base).expect("remove junction test root");
    }

    #[cfg(windows)]
    #[test]
    fn local_list_objects_terminates_on_junction_cycle() {
        const CHILD_ROOT_ENV: &str = "NEURO_GATEWAY_OBJECT_STORAGE_CYCLE_ROOT";

        if let Some(root) = std::env::var_os(CHILD_ROOT_ENV) {
            let storage = GatewayObjectStorage {
                driver: ObjectStorageDriver::Local {
                    root: PathBuf::from(root),
                },
            };
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build cycle-test runtime");
            let objects = runtime
                .block_on(storage.list_objects(""))
                .expect("junction cycle must not break object listing");
            assert_eq!(objects, vec!["object.json"]);
            return;
        }

        let base =
            std::env::temp_dir().join(format!("neuro-gateway-object-cycle-{}", Uuid::new_v4()));
        let root = base.join("root");
        let junction = root.join("cycle");
        std::fs::create_dir_all(&root).expect("create cycle-test object root");
        std::fs::write(root.join("object.json"), b"payload").expect("write cycle-test object");
        let junction_output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&root)
            .output()
            .expect("create cycle-test junction");
        assert!(
            junction_output.status.success(),
            "failed to create cycle-test junction: {}",
            String::from_utf8_lossy(&junction_output.stderr)
        );

        let mut child = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "object_storage::tests::local_list_objects_terminates_on_junction_cycle",
                "--nocapture",
            ])
            .env(CHILD_ROOT_ENV, &root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn isolated cycle-test child");

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let completed = loop {
            if child
                .try_wait()
                .expect("poll isolated cycle-test child")
                .is_some()
            {
                break true;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().expect("stop hung cycle-test child");
                break false;
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        let output = child
            .wait_with_output()
            .expect("collect isolated cycle-test child output");

        std::fs::remove_dir(&junction).expect("remove cycle-test junction");
        std::fs::remove_dir_all(&base).expect("remove cycle-test root");

        assert!(
            completed && output.status.success(),
            "junction cycle listing did not terminate successfully\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

async fn probe_local_object_storage(root: &Path, deadline: Duration) -> ObjectStorageProbeOutcome {
    match timeout(deadline, fs::create_dir_all(root)).await {
        Ok(Ok(())) => {}
        Ok(Err(_)) => {
            return ObjectStorageProbeOutcome {
                ready: false,
                timed_out: false,
            }
        }
        Err(_) => {
            return ObjectStorageProbeOutcome {
                ready: false,
                timed_out: true,
            }
        }
    }

    let probe_path = root.join(format!(".neuro-gateway-readiness-{}.tmp", Uuid::new_v4()));
    let probe_payload = b"neuro-gateway-object-storage-readiness-v1";

    let write = timeout(deadline, fs::write(&probe_path, probe_payload)).await;
    let write_ready = matches!(write, Ok(Ok(())));
    let mut timed_out = write.is_err();

    let read_ready = if write_ready {
        match timeout(deadline, fs::read(&probe_path)).await {
            Ok(Ok(payload)) => payload == probe_payload,
            Ok(Err(_)) => false,
            Err(_) => {
                timed_out = true;
                false
            }
        }
    } else {
        false
    };

    let cleanup_ready = match timeout(deadline, fs::remove_file(&probe_path)).await {
        Ok(Ok(())) => true,
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => true,
        Ok(Err(_)) => false,
        Err(_) => {
            timed_out = true;
            false
        }
    };

    ObjectStorageProbeOutcome {
        ready: write_ready && read_ready && cleanup_ready,
        timed_out,
    }
}
