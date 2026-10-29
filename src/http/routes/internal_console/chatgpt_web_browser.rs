//! Explicit local Web-session capture, independent of Codex OAuth and runtime recovery.
use super::{console_request_context, json_with_no_store, required_console_management_token};
use crate::{error::GatewayError, http::extractors::OptionalBearerToken, state::AppState};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;

static CAPTURE_SLOT: Semaphore = Semaphore::const_new(1);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureRequest {
    provider_id: String,
    expected_revision: String,
    #[serde(default)]
    request_id: Option<String>,
    #[serde(flatten)]
    capture: CaptureInput,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureInput {
    operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_id: Option<String>,
}

impl CaptureInput {
    fn validate(&self) -> Result<(), GatewayError> {
        if matches!(
            self.operation.as_str(),
            "login" | "cancel" | "status" | "verify"
        ) {
            return if self.port.is_none() && self.target_id.is_none() {
                Ok(())
            } else {
                Err(GatewayError::bad_request(
                    "Login does not accept browser ports or pages",
                ))
            };
        }
        let target_valid = self.target_id.as_ref().is_some_and(|id| {
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        });
        if self
            .port
            .is_none_or(|port| port < 1024 || [4200, 4300].contains(&port))
            || !matches!(self.operation.as_str(), "list" | "prepare")
            || (self.operation == "prepare" && !target_valid)
            || (self.operation == "list" && self.target_id.is_some())
        {
            return Err(GatewayError::bad_request(
                "Invalid browser port or page selection",
            ));
        }
        Ok(())
    }
}

fn worker_script() -> Option<PathBuf> {
    let name = "scripts/chatgpt-web-browser-import-worker.mjs";
    let mut roots = Vec::new();
    if let Some(root) = std::env::var_os("GATEWAY_RELEASE_PAYLOAD_ROOT") {
        roots.push(PathBuf::from(root));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    roots
        .into_iter()
        .map(|root| root.join(name))
        .find(|path| path.is_file())
}

fn worker_failure(code: Option<&str>) -> GatewayError {
    let stage = match code {
        Some("browser_timeout") => "login_window",
        Some("browser_capture_start_timeout") => "capture_start",
        Some("session_read_timeout") => "session_read",
        Some("browser_work_timeout") => "work_deadline",
        _ => "capture",
    };
    tracing::warn!(stage, "ChatGPT web account capture did not complete");
    let message = match code {
        Some("browser_closed") => "登录窗口已关闭，请重新点击网页登录。",
        Some("browser_timeout") => "等待登录窗口关闭超时；完成登录后请关闭该专用窗口。",
        Some("browser_capture_start_timeout") => "登录窗口已关闭，但账号读取窗口未能就绪。",
        Some("browser_work_timeout") => "账号导入处理超时，未能完成读取。",
        Some("verification_wait_timeout") => "等待人工验证操作超时，尚未读取账号。",
        Some("verification_prepare_failed") => {
            "会话接口要求官网验证，但读取窗口未能安全退出；本次导入已停止。"
        }
        Some("session_read_timeout") => "登录窗口已关闭，但读取账号会话超时，未能导入。",
        Some("browser_not_found") => "未找到 Chrome 或 Edge，请安装浏览器后重试。",
        Some("login_required") => "请在所选 ChatGPT 页面完成登录或人工验证后重试。",
        Some("session_page_unavailable") => "ChatGPT 页面加载或跳转尚未完成，未能读取账号。",
        Some("session_request_failed") => {
            "已打开 ChatGPT 页面，但网页会话请求失败；请检查浏览器网络后重试。"
        }
        Some("session_verification_required") => {
            "ChatGPT 会话接口要求浏览器验证，尚未读取账号；这不代表网页登录失败。"
        }
        Some("session_access_forbidden") => {
            "ChatGPT 会话接口拒绝访问（HTTP 403）；尚不能据此判断登录状态或模型权限。"
        }
        Some("session_rate_limited") => "ChatGPT 会话接口请求受限（HTTP 429），请稍后重试。",
        Some("session_service_unavailable") => {
            "ChatGPT 会话接口暂时不可用（HTTP 5xx），请稍后重试。"
        }
        Some("session_response_not_json") => {
            "ChatGPT 会话接口返回了非 JSON 页面，可能仍在加载或等待人工验证。"
        }
        Some("session_response_invalid") => "ChatGPT 网页会话响应格式异常，未能读取账号。",
        Some("session_token_invalid") => "已收到网页会话，但账号令牌格式不受支持，未能读取账号。",
        Some("session_expired") => "读取到的 ChatGPT 网页会话已过期，请重新登录。",
        Some("tab_unavailable") => "所选 ChatGPT 页面已关闭或地址变化，请重新读取页面列表。",
        Some("incomplete_session") => "网页会话材料尚不完整，请在 ChatGPT 页面刷新后重试。",
        _ => "无法完成网页登录，请重试；不会重启您正在使用的浏览器。",
    };
    GatewayError::service_unavailable(message).with_code("chatgpt_web_browser_capture_failed")
}

pub async fn capture_web_browser(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CaptureRequest>,
) -> Result<Response, GatewayError> {
    let context = console_request_context(&headers, info.as_ref());
    let actor = state.console_auth.authenticate_management_token(
        &context,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    state.console_auth.verify_secret_grant(
        &context,
        &actor,
        headers
            .get("x-secret-grant")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default(),
    )?;
    // Never let forwarded headers or an absent peer identity grant access to local browser sessions.
    if !info.as_ref().is_some_and(|peer| peer.0.ip().is_loopback()) {
        let mut error =
            GatewayError::unauthorized("Browser capture requires a direct loopback connection");
        error.http_status = Some(403);
        return Err(error);
    }
    body.capture.validate()?;
    if matches!(
        body.capture.operation.as_str(),
        "login" | "cancel" | "status" | "verify"
    ) && !body
        .request_id
        .as_ref()
        .is_some_and(|id| id.len() == 36 && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-'))
    {
        return Err(GatewayError::bad_request(
            "Invalid login request identifier",
        ));
    }
    // Cancellation still requires authentication and direct loopback, but not a stale revision.
    if body.capture.operation == "cancel" {
        super::chatgpt_web_login::cancel(body.request_id.as_deref().unwrap_or_default());
        return Ok(json_with_no_store(serde_json::json!({"cancelled": true})));
    }
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != body.expected_revision {
        return Err(GatewayError::conflict(
            "Configuration changed; reopen the account editor",
        ));
    }
    if !snapshot
        .document()
        .providers
        .iter()
        .any(|p| p.id == body.provider_id && p.preset.as_deref() == Some("chatgpt-web-reverse"))
    {
        return Err(GatewayError::bad_request(
            "Browser capture requires a saved chatgpt-web provider",
        ));
    }
    let owner = super::chatgpt_web_login::LoginOwner {
        actor: actor.token_fingerprint().to_owned(),
        provider: body.provider_id.clone(),
        revision: body.expected_revision.clone(),
    };
    if matches!(body.capture.operation.as_str(), "status" | "verify") {
        let channel = super::chatgpt_web_login::interaction(
            body.request_id.as_deref().unwrap_or_default(),
            &owner,
        )
        .ok_or_else(|| GatewayError::conflict("该网页登录已结束或不属于当前账号编辑器。"))?;
        if body.capture.operation == "verify" {
            channel.verify().await.map_err(|_| {
                GatewayError::conflict("当前没有可继续的人工验证，或已提交过验证操作。")
            })?;
            return Ok(json_with_no_store(
                serde_json::json!({"phase": "verification_window"}),
            ));
        }
        let phase = channel
            .phase()
            .await
            .map_err(|_| GatewayError::conflict("该网页登录已结束，无法继续读取状态。"))?;
        return Ok(json_with_no_store(serde_json::json!({"phase": phase})));
    }
    let _permit = CAPTURE_SLOT
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Another browser capture is running"))?;
    let script_path = worker_script().ok_or_else(|| worker_failure(None))?;
    let node_bin = std::env::var("CHATGPT_WEB_BROWSER_NODE_BIN").unwrap_or_else(|_| "node".into());
    let stdin_json = serde_json::to_vec(&body.capture).map_err(|_| worker_failure(None))?;
    let workspace = crate::upstream::ConsoleBrowserWorkspace::create()
        .await
        .map_err(|_| worker_failure(None))?;
    let (_login_guard, mut cancelled) = super::chatgpt_web_login::LoginGuard::begin(
        body.request_id.unwrap_or_default(),
        owner,
        workspace.interaction(),
    );
    let worker = crate::upstream::execute_console_browser_worker(
        crate::upstream::PreparedProducerBrowserWorkerLaunch {
            node_bin,
            script_path,
            stdin_json,
        },
        workspace,
    );
    let output = tokio::select! {
        biased;
        _ = cancelled.changed() => return Err(GatewayError::bad_request("网页登录已取消。")),
        output = worker => output.map_err(|_| worker_failure(None))?,
    };
    let mut output: Value =
        serde_json::from_slice(&output.stdout).map_err(|_| worker_failure(None))?;
    if output.get("cleanupPending").and_then(Value::as_bool) == Some(true) {
        // The supervisor already reaped its owned tree; no page text or credentials enter logs.
        tracing::warn!(
            code = "chatgpt_web_browser_cleanup_pending",
            "Browser cleanup needed the parent supervisor; capture outcome was preserved"
        );
    }
    if output.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(worker_failure(output.get("code").and_then(Value::as_str)));
    }
    // Auth and source revision may change during the browser read; discard stale secret material.
    state.console_auth.verify_secret_grant(
        &context,
        &actor,
        headers
            .get("x-secret-grant")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default(),
    )?;
    if state.route_config.snapshot().revision().id() != body.expected_revision {
        return Err(GatewayError::conflict(
            "Configuration changed during browser capture",
        ));
    }
    if let Some(object) = output.as_object_mut() {
        object.remove("ok");
    }
    Ok(json_with_no_store(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_failures_distinguish_session_reads_without_echoing_worker_text() {
        for (code, detail) in [
            ("browser_capture_start_timeout", "读取窗口未能就绪"),
            ("browser_work_timeout", "导入处理超时"),
            ("verification_wait_timeout", "人工验证操作超时"),
            ("verification_prepare_failed", "读取窗口未能安全退出"),
            ("session_read_timeout", "读取账号会话超时"),
            ("session_page_unavailable", "页面加载"),
            ("session_request_failed", "请求失败"),
            ("session_verification_required", "浏览器验证"),
            ("session_access_forbidden", "HTTP 403"),
            ("session_rate_limited", "HTTP 429"),
            ("session_service_unavailable", "HTTP 5xx"),
            ("session_response_not_json", "非 JSON"),
            ("session_token_invalid", "格式不受支持"),
            ("session_expired", "已过期"),
        ] {
            let error = worker_failure(Some(code));
            assert!(error.message.contains(detail));
            assert!(!error.message.contains("请在所选"));
            assert_eq!(
                error.code.as_deref(),
                Some("chatgpt_web_browser_capture_failed")
            );
        }
        assert!(!worker_failure(Some("private worker detail"))
            .message
            .contains("private"));
    }

    #[test]
    fn validates_explicit_local_capture_without_oauth_or_arbitrary_hosts() {
        for body in [
            serde_json::json!({"providerId":"web","expectedRevision":"r","operation":"login","requestId":"11111111-1111-1111-1111-111111111111"}),
            serde_json::json!({"providerId":"web","expectedRevision":"r","operation":"cancel","requestId":"11111111-1111-1111-1111-111111111111"}),
            serde_json::json!({"providerId":"web","expectedRevision":"r","operation":"status","requestId":"11111111-1111-1111-1111-111111111111"}),
            serde_json::json!({"providerId":"web","expectedRevision":"r","operation":"verify","requestId":"11111111-1111-1111-1111-111111111111"}),
            serde_json::json!({"providerId":"web","expectedRevision":"r","operation":"list","port":9222}),
            serde_json::json!({"providerId":"web","expectedRevision":"r","operation":"prepare","port":9222,"targetId":"ABC-123"}),
        ] {
            serde_json::from_value::<CaptureRequest>(body)
                .unwrap()
                .capture
                .validate()
                .unwrap();
        }
        for input in [
            CaptureInput {
                operation: "login".into(),
                port: Some(9222),
                target_id: None,
            },
            CaptureInput {
                operation: "prepare".into(),
                port: Some(9222),
                target_id: None,
            },
            CaptureInput {
                operation: "list".into(),
                port: Some(4200),
                target_id: None,
            },
        ] {
            assert!(input.validate().is_err());
        }
        assert!(serde_json::from_value::<CaptureRequest>(serde_json::json!({"providerId":"web","expectedRevision":"r","operation":"list","port":9222,"host":"remote"})).is_err());
    }
}
