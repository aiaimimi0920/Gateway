//! 管理目录的一次性 owned runtime；任务由 Gateway 统一 join，不随 HTTP future 被丢弃。
use crate::{error::GatewayError, state::GatewayShutdownHandle};
use gateway_copilot_runtime::{CopilotRuntime, RuntimeConfig};
use serde_json::Value;
use std::{path::PathBuf, time::Duration};
use tokio::sync::oneshot;

#[derive(Clone)]
pub(crate) struct Settings {
    pub executable: PathBuf,
    pub workspace_root: PathBuf,
}

pub(crate) struct RevisionFence {
    pub store: std::sync::Arc<crate::routing::config::RouteConfigStore>,
    pub revision: String,
}

impl RevisionFence {
    fn current(&self) -> bool {
        self.store.snapshot().revision().id() == self.revision
    }
}

impl Settings {
    pub(crate) fn from_env() -> Option<Self> {
        Some(Self {
            executable: std::env::var_os("GATEWAY_COPILOT_CLI")?.into(),
            workspace_root: std::env::var_os("GATEWAY_COPILOT_WORKSPACE_ROOT")?.into(),
        })
    }
}

pub(crate) fn unavailable() -> GatewayError {
    GatewayError::service_unavailable("Copilot catalog is unavailable")
        .with_code("copilot_catalog_unavailable")
}

/// 仅清理本任务新建的随机目录；启动／关闭未确认时保留现场，不删仍可能使用中的文件。
pub(crate) async fn run(
    settings: Settings,
    token: String,
    shutdown: GatewayShutdownHandle,
    reply: &mut oneshot::Sender<Result<Value, GatewayError>>,
    fence: RevisionFence,
) -> Result<Value, GatewayError> {
    if !settings.executable.is_absolute() || !settings.workspace_root.is_absolute() {
        return Err(unavailable());
    }
    let root = tokio::fs::canonicalize(&settings.workspace_root)
        .await
        .map_err(|_| unavailable())?;
    if !root.is_dir() || !settings.executable.is_file() {
        return Err(unavailable());
    }
    if shutdown.is_requested() || reply.is_closed() || !fence.current() {
        return Err(unavailable());
    }
    let directory = root.join(format!("copilot-catalog-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir(&directory)
        .await
        .map_err(|_| unavailable())?;
    let home = directory.join("home");
    tokio::fs::create_dir(&home)
        .await
        .map_err(|_| unavailable())?;
    // SDK owns child startup/reaping. Do not race an independent process killer with it.
    let owner = tokio::time::timeout(
        Duration::from_secs(15),
        CopilotRuntime::start(RuntimeConfig {
            executable: settings.executable,
            directory: home,
        }),
    )
    .await
    .map_err(|_| unavailable())?
    .map_err(|_| unavailable())?;
    // Startup may await external work; reject a rotated/deleted identity before sending it.
    let result = if !fence.current() {
        Err(unavailable())
    } else {
        tokio::select! {
            biased;
            _ = shutdown.wait() => Err(unavailable()),
            _ = reply.closed() => Err(unavailable()),
            result = tokio::time::timeout(Duration::from_secs(30), owner.list_models_for_token(token.clone())) => {
                result.map_err(|_| unavailable()).and_then(|r| r.map_err(|_| unavailable()))
            }
        }
    };
    // Keep the admission slot until shutdown and cleanup finish, including disconnected callers.
    tokio::time::timeout(Duration::from_secs(40), owner.stop())
        .await
        .map_err(|_| unavailable())?
        .map_err(|_| unavailable())?;
    tokio::fs::remove_dir_all(&directory)
        .await
        .map_err(|_| unavailable())?;
    let models = result?;
    if models.len() > 4096 {
        return Err(unavailable());
    }
    let catalog = serde_json::to_value(models).map_err(|_| unavailable())?;
    let encoded = serde_json::to_string(&catalog).map_err(|_| unavailable())?;
    // Fail closed rather than editing upstream identifiers or echoing explicit secret material.
    if encoded.len() > 1024 * 1024 || contains_secret(&catalog, &token) {
        return Err(unavailable());
    }
    Ok(catalog)
}

// Inspect decoded strings, including object keys, so JSON escaping cannot hide a token.
fn contains_secret(value: &Value, token: &str) -> bool {
    match value {
        Value::String(value) => value.contains(token),
        Value::Array(values) => values.iter().any(|value| contains_secret(value, token)),
        Value::Object(values) => values
            .iter()
            .any(|(key, value)| key.contains(token) || contains_secret(value, token)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copilot_catalog_rejects_secrets_in_decoded_strings_and_keys() {
        let token = "catalog-\"\\secret";
        for value in [
            serde_json::json!({"models": [{"name": format!("prefix {token} suffix")}]}),
            serde_json::json!({token: true}),
        ] {
            assert!(!serde_json::to_string(&value).unwrap().contains(token));
            assert!(contains_secret(&value, token));
        }
        assert!(!contains_secret(
            &serde_json::json!({"models": ["safe", 1, null]}),
            token
        ));
    }
}
