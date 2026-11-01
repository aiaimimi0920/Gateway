//! FlowMusic's Supabase session is a credential, not Google cookies or an official API key.
use super::{
    now_rfc3339, CreateGeminiAuthSessionInput, GeminiAuthFamily, GeminiAuthSessionManager,
    GeminiAuthSessionStatus, GeminiAuthSessionView, GeminiGeneratedCredentialDraft,
};
use crate::{
    error::GatewayError,
    upstream::{execute_producer_browser_worker_process, PreparedProducerBrowserWorkerLaunch},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tokio::sync::Semaphore;
use uuid::Uuid;

static IMPORT_SLOT: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Material {
    schema_version: u8,
    provider: String,
    access_token: String,
    refresh_token: Option<String>,
}

fn invalid() -> GatewayError {
    GatewayError::bad_request("FlowMusic 会话 JSON 无效；仅接受该服务的 accessToken 和可选 refreshToken，不接受完整浏览器状态。")
        .with_code("console_flowmusic_session_invalid")
}

fn prepare(
    input: CreateGeminiAuthSessionInput,
    source: &str,
) -> Result<(GeminiAuthSessionView, String, Vec<u8>), GatewayError> {
    if input.target_family != GeminiAuthFamily::FlowmusicWeb || source.len() > 24 * 1024 {
        return Err(invalid());
    }
    let material: Material = serde_json::from_str(source).map_err(|_| invalid())?;
    let valid_token = |token: &str, max| {
        !token.is_empty()
            && token.len() <= max
            && token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    };
    if material.schema_version != 1
        || material.provider != "flowmusic-web"
        || !valid_token(&material.access_token, 16384)
        || material
            .refresh_token
            .as_ref()
            .is_some_and(|token| !valid_token(token, 4096))
    {
        return Err(invalid());
    }
    let id = Uuid::new_v4().to_string();
    let key = format!("credential-runtime/flowmusic/{id}/http-session.json");
    let bytes = serde_json::to_vec(&material).map_err(|_| invalid())?;
    let now = now_rfc3339();
    Ok((
        GeminiAuthSessionView {
            id: id.clone(),
            target_family: input.target_family,
            provider_id: input.provider_id.clone(),
            status: GeminiAuthSessionStatus::Succeeded,
            message: "FlowMusic 登录材料已导入草稿；保存后生效，尚未验证音乐生成。".into(),
            created_at: now.clone(),
            updated_at: now,
            generated_drafts: vec![GeminiGeneratedCredentialDraft {
                provider_id: input.provider_id,
                credential: json!({"id":format!("flowmusic-web-{id}"),"account_name":input.account_label.as_deref().unwrap_or("FlowMusic Web"),
                "enabled":true,"runtime_state_object_key":key,"supported_models":["producer:standard"],"model_selection":{"fetched":[],"manual":["producer:standard"]},"scheduled_probe_enabled":false}),
                secret_edits: vec![],
            }],
        },
        key,
        bytes,
    ))
}

pub async fn import(
    input: CreateGeminiAuthSessionInput,
    source: &str,
) -> Result<GeminiAuthSessionView, GatewayError> {
    let (session, key, bytes) = prepare(input, source)?;
    let failed = || {
        GatewayError::service_unavailable("无法保存 FlowMusic 凭据，请检查存储配置。")
            .with_code("flowmusic_session_storage_failed")
    };
    crate::object_storage::gateway_object_storage()
        .map_err(|_| failed())?
        .put_bytes(&key, bytes, "application/json")
        .await
        .map_err(|_| failed())?;
    Ok(session)
}

impl GeminiAuthSessionManager {
    pub(super) fn create_flowmusic_session(
        self: &Arc<Self>,
        input: CreateGeminiAuthSessionInput,
    ) -> GeminiAuthSessionView {
        let now = now_rfc3339();
        let mut session = GeminiAuthSessionView {
            id: Uuid::new_v4().to_string(),
            target_family: input.target_family,
            provider_id: input.provider_id.clone(),
            status: GeminiAuthSessionStatus::WaitingUser,
            message: "请在普通浏览器中登录 FlowMusic，进入主界面后关闭本次专用窗口；无需生成音乐。"
                .into(),
            created_at: now.clone(),
            updated_at: now,
            generated_drafts: vec![],
        };
        let permit = IMPORT_SLOT.clone().try_acquire_owned();
        if !cfg!(windows) || permit.is_err() {
            session.status = GeminiAuthSessionStatus::Failed;
            session.message =
                "FlowMusic 登录导入需要 Windows 本地浏览器，且同一时间只能导入一个账号。".into();
        }
        // Retain at most 32 receipts for this family; never evict an in-flight import.
        let mut finished: Vec<_> = self
            .sessions
            .iter()
            .filter(|s| {
                s.target_family == GeminiAuthFamily::FlowmusicWeb
                    && matches!(
                        s.status,
                        GeminiAuthSessionStatus::Succeeded | GeminiAuthSessionStatus::Failed
                    )
            })
            .map(|s| (s.id.clone(), s.updated_at.clone()))
            .collect();
        finished.sort_by(|a, b| a.1.cmp(&b.1));
        let remove = finished.len().saturating_sub(30);
        for (id, _) in finished.into_iter().take(remove) {
            self.sessions.remove(&id);
        }
        self.sessions.insert(session.id.clone(), session.clone());
        if session.status == GeminiAuthSessionStatus::WaitingUser {
            let manager = Arc::clone(self);
            let id = session.id.clone();
            tokio::spawn(async move {
                let _permit = permit.expect("admitted FlowMusic import");
                match capture(input).await {
                    Ok(result) => manager.update_session(
                        &id,
                        result.status,
                        result.message,
                        result.generated_drafts,
                    ),
                    Err(()) => manager.update_session(
                        &id,
                        GeminiAuthSessionStatus::Failed,
                        "FlowMusic 登录导入失败或超时；私人详情已省略，请重新尝试。".into(),
                        vec![],
                    ),
                }
            });
        }
        session
    }
}

async fn capture(input: CreateGeminiAuthSessionInput) -> Result<GeminiAuthSessionView, ()> {
    let temp = std::env::var_os("FLOWMUSIC_CAPTURE_TEMP_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            gateway_local_data::selected_root()
                .ok()
                .map(|root| root.join("runtime/flowmusic-import"))
        })
        .filter(|path| path.is_absolute())
        .ok_or(())?;
    let script = std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()
                .map(|p| p.join("scripts/flowmusic-auth-worker.mjs"))
        })
        .filter(|path| path.is_file())
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/flowmusic-auth-worker.mjs")
        });
    let output = execute_producer_browser_worker_process(
        PreparedProducerBrowserWorkerLaunch {
            node_bin: std::env::var("FLOWMUSIC_NODE_BIN").unwrap_or_else(|_| "node".into()),
            script_path: script,
            stdin_json: serde_json::to_vec(&json!({"tempRoot":temp})).map_err(|_| ())?,
        },
        Duration::from_secs(25 * 60),
    )
    .await
    .map_err(|_| ())?;
    if output.stdout.len() > 24 * 1024 {
        return Err(());
    }
    import(input, std::str::from_utf8(&output.stdout).map_err(|_| ())?)
        .await
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> CreateGeminiAuthSessionInput {
        CreateGeminiAuthSessionInput {
            target_family: GeminiAuthFamily::FlowmusicWeb,
            provider_id: "chosen".into(),
            account_label: Some("手测".into()),
        }
    }
    #[test]
    fn session_secrets_are_separated_and_models_are_manual_music_only() {
        let (receipt, _, bytes) = prepare(input(), r#"{"schemaVersion":1,"provider":"flowmusic-web","accessToken":"fixture.access.token","refreshToken":"refresh-fixture"}"#).unwrap();
        let draft = &receipt.generated_drafts[0];
        assert_eq!(
            draft.credential["supported_models"],
            json!(["producer:standard"])
        );
        assert!(!draft.credential.to_string().contains("fixture"));
        assert!(draft.secret_edits.is_empty());
        assert!(String::from_utf8(bytes)
            .unwrap()
            .contains("refresh-fixture"));
        assert_eq!(draft.provider_id, "chosen");
    }
    #[test]
    fn rejects_foreign_state_injection_and_oversized_json() {
        for source in [
            r#"{"schemaVersion":1,"provider":"gemini-web","accessToken":"fixture"}"#,
            r#"{"schemaVersion":1,"provider":"flowmusic-web","accessToken":"a\r\nb"}"#,
            r#"{"schemaVersion":1,"provider":"flowmusic-web","accessToken":"fixture","origins":[]}"#,
        ] {
            assert!(prepare(input(), source).is_err());
        }
        assert!(prepare(input(), &" ".repeat(24577)).is_err());
    }
}
