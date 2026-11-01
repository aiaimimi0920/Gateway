//! Flow's ordinary login import persists Cookie-only material after read-only HTTP verification.
use super::{
    now_rfc3339, CreateGeminiAuthSessionInput, GeminiAuthFamily, GeminiAuthSessionManager,
    GeminiAuthSessionStatus, GeminiAuthSessionView,
};
use crate::upstream::{
    execute_producer_browser_worker_process, PreparedProducerBrowserWorkerLaunch,
};
use std::{
    path::PathBuf,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tokio::sync::Semaphore;
use uuid::Uuid;

static IMPORT_SLOT: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));

impl GeminiAuthSessionManager {
    pub(super) fn create_flow_session(
        self: &Arc<Self>,
        input: CreateGeminiAuthSessionInput,
    ) -> GeminiAuthSessionView {
        let now = now_rfc3339();
        let mut session = GeminiAuthSessionView {
            id: Uuid::new_v4().to_string(),
            target_family: GeminiAuthFamily::FlowWeb,
            provider_id: input.provider_id.clone(),
            status: GeminiAuthSessionStatus::WaitingUser,
            message:
                "请在普通浏览器中登录 Google Flow，进入主界面后关闭本次专用窗口；无需生成内容。"
                    .into(),
            created_at: now.clone(),
            updated_at: now,
            generated_drafts: vec![],
        };
        let permit = IMPORT_SLOT.clone().try_acquire_owned();
        if !cfg!(windows) || permit.is_err() {
            session.status = GeminiAuthSessionStatus::Failed;
            session.message =
                "Flow 登录导入需要 Windows 本地浏览器，同一时间只能导入一个账号。".into();
        }
        let mut finished: Vec<_> = self
            .sessions
            .iter()
            .filter(|s| {
                s.target_family == GeminiAuthFamily::FlowWeb
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
                let _permit = permit.expect("admitted Flow import");
                match capture(input).await {
                    Ok(result) => manager.update_session(&id, result.status,
                        "Flow 凭据已导入且 HTTP 只读额度请求通过；图片/视频生成仍受 fresh proof 阻塞，未发送生成请求。".into(), result.generated_drafts),
                    Err(()) => manager.update_session(&id, GeminiAuthSessionStatus::Failed,
                        "Flow 登录导入或 HTTP 只读验证失败；私人详情已省略，请重新尝试。".into(), vec![]),
                }
            });
        }
        session
    }
}

async fn capture(input: CreateGeminiAuthSessionInput) -> Result<GeminiAuthSessionView, ()> {
    let temp = std::env::var_os("FLOW_CAPTURE_TEMP_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            gateway_local_data::selected_root()
                .ok()
                .map(|root| root.join("runtime/flow-import"))
        })
        .filter(|path| path.is_absolute())
        .ok_or(())?;
    let script = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("scripts/flow-auth-worker.mjs")))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/flow-auth-worker.mjs")
        });
    let output = execute_producer_browser_worker_process(
        PreparedProducerBrowserWorkerLaunch {
            node_bin: std::env::var("FLOW_HTTP_NODE_BIN").unwrap_or_else(|_| "node".into()),
            script_path: script,
            stdin_json: serde_json::to_vec(&serde_json::json!({"tempRoot":temp}))
                .map_err(|_| ())?,
        },
        Duration::from_secs(25 * 60),
    )
    .await
    .map_err(|_| ())?;
    let source = std::str::from_utf8(&output.stdout).map_err(|_| ())?;
    let bytes = crate::console::google_web_json::validate_flow_bundle(source).map_err(|_| ())?;
    let client = crate::http_client::builder()
        .redirect(rquest::redirect::Policy::none())
        .build()
        .map_err(|_| ())?;
    let refreshed = crate::upstream::flow_web::verify_import_bundle(&client, &bytes)
        .await
        .map_err(|_| ())?;
    let source = std::str::from_utf8(&refreshed).map_err(|_| ())?;
    crate::console::google_web_json::import(input, source)
        .await
        .map_err(|_| ())
}
