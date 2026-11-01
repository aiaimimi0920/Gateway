//! Interactive AI Studio login. Only the credential owner persists the private pipe result.
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Semaphore;
use uuid::Uuid;

use super::{
    now_rfc3339, CreateGeminiAuthSessionInput, GeminiAuthFamily, GeminiAuthSessionManager,
    GeminiAuthSessionStatus, GeminiAuthSessionView, GeminiGeneratedCredentialDraft,
};
use crate::object_storage::gateway_object_storage;
use crate::upstream::{
    execute_producer_browser_worker_process, PreparedProducerBrowserWorkerLaunch,
};

static IMPORT_SLOT: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));
const SUPPORTED_MODELS: [&str; 4] = [
    "gemini-3.5-flash-lite",
    "gemini-3.5-transcribe",
    "gemini-3.8-flash-lite-tts",
    "gemini-3.1-flash-lite-image",
];

#[derive(Deserialize)]
struct CaptureOutput {
    ok: bool,
    bundle: Value,
    models: Vec<String>,
}

fn parse_capture(bytes: &[u8]) -> Result<(Vec<u8>, Vec<String>), String> {
    let invalid = || "AI Studio 导入结果无效，私人详情已省略。".to_string();
    if bytes.len() > 64 * 1024 {
        return Err(invalid());
    }
    let output: CaptureOutput = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    let bundle = &output.bundle;
    let scope = bundle.get("authUser").and_then(Value::as_str).unwrap_or("");
    let cookies = bundle.get("cookies").and_then(Value::as_array);
    if !output.ok
        || bundle.get("schemaVersion").and_then(Value::as_u64) != Some(1)
        || bundle.get("provider").and_then(Value::as_str) != Some("aistudio-web")
        || scope.is_empty()
        || scope.len() > 3
        || !scope.bytes().all(|c| c.is_ascii_digit())
        || bundle.as_object().is_none_or(|map| map.len() != 4)
        || cookies.is_none_or(|cookies| cookies.is_empty() || cookies.len() > 64)
        || output.models.len() > 256
        || output.models.iter().any(|id| {
            id.is_empty()
                || id.len() > 128
                || !id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        })
    {
        return Err(invalid());
    }
    let bytes = serde_json::to_vec(bundle).map_err(|_| invalid())?;
    let models: Vec<String> = SUPPORTED_MODELS
        .iter()
        .filter(|id| output.models.iter().any(|m| m.as_str() == **id))
        .map(|id| id.to_string())
        .collect();
    if bytes.len() > 32768 || models.is_empty() {
        return Err(invalid());
    }
    Ok((bytes, models))
}

fn script_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()
                .map(|dir| dir.join("scripts/aistudio-auth-worker.mjs"))
        })
        .filter(|path| path.is_file())
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/aistudio-auth-worker.mjs")
        })
}

impl GeminiAuthSessionManager {
    pub(super) fn create_aistudio_session(
        self: &Arc<Self>,
        input: CreateGeminiAuthSessionInput,
    ) -> GeminiAuthSessionView {
        // Bound this new family's retained receipts; never prune other providers' sessions.
        let mut finished: Vec<_> = self
            .sessions
            .iter()
            .filter(|entry| {
                entry.target_family == GeminiAuthFamily::AistudioWeb
                    && matches!(
                        entry.status,
                        GeminiAuthSessionStatus::Succeeded | GeminiAuthSessionStatus::Failed
                    )
            })
            .map(|entry| (entry.id.clone(), entry.updated_at.clone()))
            .collect();
        finished.sort_by(|a, b| a.1.cmp(&b.1));
        let remove = finished.len().saturating_sub(30);
        for (id, _) in finished.into_iter().take(remove) {
            self.sessions.remove(&id);
        }
        let now = now_rfc3339();
        let mut session = GeminiAuthSessionView { id: Uuid::new_v4().to_string(), target_family: GeminiAuthFamily::AistudioWeb,
            provider_id: input.provider_id.trim().to_string(), status: GeminiAuthSessionStatus::WaitingUser,
            message: "请在新开的普通浏览器中登录 AI Studio，进入对话界面后关闭本次专用窗口；随后自动导入，无需发送提示。".into(),
            created_at: now.clone(), updated_at: now, generated_drafts: Vec::new() };
        let permit = IMPORT_SLOT.clone().try_acquire_owned();
        if !cfg!(windows) || permit.is_err() {
            session.status = GeminiAuthSessionStatus::Failed;
            session.message = if cfg!(windows) {
                "已有 AI Studio 登录导入正在运行，请先完成或关闭该专用窗口。"
            } else {
                "此导入入口需要 Windows 本地桌面浏览器，尚未接入远程宿主。"
            }
            .into();
        }
        self.sessions.insert(session.id.clone(), session.clone());
        if session.status == GeminiAuthSessionStatus::WaitingUser {
            let manager = Arc::clone(self);
            let id = session.id.clone();
            tokio::spawn(async move {
                let _permit = permit.expect("admitted interactive import");
                let result = manager.capture_aistudio(&id, &input).await;
                match result {
                    Ok(draft) => manager.update_session(
                        &id,
                        GeminiAuthSessionStatus::Succeeded,
                        "AI Studio 登录凭据已导入，未发送生成请求。".into(),
                        vec![draft],
                    ),
                    Err(message) => manager.update_session(
                        &id,
                        GeminiAuthSessionStatus::Failed,
                        message,
                        Vec::new(),
                    ),
                }
            });
        }
        session
    }

    async fn capture_aistudio(
        &self,
        id: &str,
        input: &CreateGeminiAuthSessionInput,
    ) -> Result<GeminiGeneratedCredentialDraft, String> {
        let fail = || "AI Studio 登录导入失败或超时，请重新尝试；私人详情已省略。".to_string();
        let temp = std::env::var_os("AISTUDIO_CAPTURE_TEMP_ROOT")
            .map(PathBuf::from)
            .or_else(|| {
                gateway_local_data::selected_root()
                    .ok()
                    .map(|root| root.join("runtime/aistudio-import"))
            })
            .filter(|path| path.is_absolute())
            .ok_or_else(fail)?;
        let stdin_json = serde_json::to_vec(&json!({"tempRoot":temp})).map_err(|_| fail())?;
        let output = execute_producer_browser_worker_process(
            PreparedProducerBrowserWorkerLaunch {
                node_bin: std::env::var("AISTUDIO_HTTP_NODE_BIN").unwrap_or_else(|_| "node".into()),
                script_path: script_path(),
                stdin_json,
            },
            Duration::from_secs(25 * 60),
        )
        .await
        .map_err(|_| fail())?;
        let (bytes, models) = parse_capture(&output.stdout)?;
        let key = format!("credential-runtime/aistudio/{id}/http-cookies.json");
        gateway_object_storage()
            .map_err(|_| fail())?
            .put_bytes(&key, bytes, "application/json")
            .await
            .map_err(|_| fail())?;
        Ok(GeminiGeneratedCredentialDraft {
            provider_id: input.provider_id.trim().into(),
            credential: json!({"id":format!("aistudio-web-{id}"), "account_name":input.account_label.as_deref().unwrap_or("AI Studio Web"),
                "enabled":true, "runtime_state_object_key":key, "supported_models":models,
                "model_selection":{"fetched":models,"manual":[]}, "scheduled_probe_enabled":false}),
            secret_edits: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_only_selects_supported_models_and_never_returns_cookie_secret_edits() {
        let bytes = serde_json::to_vec(&json!({"ok":true,"bundle":{"schemaVersion":1,"provider":"aistudio-web",
            "authUser":"0","cookies":[{"name":"fixture","value":"private"}]},"models":["gemini-3.5-flash-lite","paid-video"]})).unwrap();
        let (stored, models) = parse_capture(&bytes).unwrap();
        assert!(stored.len() < 32768);
        assert_eq!(models, vec!["gemini-3.5-flash-lite"]);
        assert!(parse_capture(&vec![b'x'; 65537]).is_err());
    }

    #[test]
    fn import_rejects_foreign_provider_full_browser_state_and_unknown_models() {
        let mut output = json!({"ok":true,"bundle":{"schemaVersion":1,"provider":"foreign","authUser":"0","cookies":[{}]},
            "models":["gemini-3.5-flash-lite"]});
        assert!(parse_capture(&serde_json::to_vec(&output).unwrap()).is_err());
        output["bundle"]["provider"] = json!("aistudio-web");
        output["bundle"]["origins"] = json!([]);
        assert!(parse_capture(&serde_json::to_vec(&output).unwrap()).is_err());
        output["bundle"].as_object_mut().unwrap().remove("origins");
        output["models"] = json!(["paid-video"]);
        assert!(parse_capture(&serde_json::to_vec(&output).unwrap()).is_err());
    }
}
