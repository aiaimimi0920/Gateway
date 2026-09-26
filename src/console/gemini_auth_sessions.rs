use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

#[path = "gemini_auth_session_drafts.rs"]
mod drafts;
#[path = "gemini_auth_session_parsing.rs"]
mod parsing;
#[path = "gemini_auth_session_workers.rs"]
mod workers;

use drafts::{
    build_business_generated_drafts, build_canvas_generated_drafts, build_web_generated_drafts,
};
#[cfg(test)]
use parsing::{
    extract_last_json_object, parse_remote_gemini_business_capture_output,
    parse_remote_gemini_canvas_capture_output, parse_remote_gemini_web_capture_output,
};
#[cfg(test)]
use workers::resolve_gemini_auth_helper_execution_mode;
use workers::{
    absolute_control_path, prepare_gemini_auth_session_control_state,
    run_gemini_business_capture_worker, run_gemini_canvas_capture_worker,
    run_gemini_web_capture_worker,
};

const GEMINI_CANVAS_CAPTURE_SCRIPT: &str = "export-gemini-canvas-storage-state.mjs";
const GEMINI_BUSINESS_CAPTURE_SCRIPT: &str = "export-gemini-business-runtime.mjs";
const GEMINI_WEB_CAPTURE_SCRIPT: &str = "export-gemini-web-runtime.mjs";
const GEMINI_CANVAS_CHAT_API_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
const DEFAULT_GEMINI_CANVAS_SHARE_ID: &str = "fe24c455a570";
const GEMINI_AUTH_HELPER_TIMEOUT_SECS: u64 = 25 * 60;
const GEMINI_AUTH_REMOTE_HELPER_TIMEOUT_SECS: u64 = GEMINI_AUTH_HELPER_TIMEOUT_SECS + 30;
const GEMINI_AUTH_REMOTE_PROVIDER: &str = "gemini-auth";
const GEMINI_AUTH_REMOTE_ENDPOINT_KIND: &str = "credential_capture";
const GEMINI_AUTH_REMOTE_EXECUTION_MODE: &str = "manual_auth";
const GEMINI_AUTH_HOST_HELPER_START_HINT: &str =
    r"Start the Windows host helper with .\tools\start-gateway-browser-executor.ps1.";
const GEMINI_AUTH_CONTROL_ROOT: &str = ".runtime/gemini-auth-sessions";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeminiAuthFamily {
    GeminiCanvas,
    GeminiCanvasChat,
    GeminiBusiness,
    GeminiWeb,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeminiAuthSessionStatus {
    Pending,
    WaitingUser,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAuthSecretEdit {
    pub field: String,
    pub operation: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGeneratedCredentialDraft {
    pub provider_id: String,
    pub credential: Value,
    pub secret_edits: Vec<GeminiAuthSecretEdit>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAuthSessionView {
    pub id: String,
    pub target_family: GeminiAuthFamily,
    pub provider_id: String,
    pub status: GeminiAuthSessionStatus,
    pub message: String,
    pub created_at: String,
    pub updated_at: String,
    pub generated_drafts: Vec<GeminiGeneratedCredentialDraft>,
}

#[derive(Clone, Debug)]
pub struct CreateGeminiAuthSessionInput {
    pub target_family: GeminiAuthFamily,
    pub provider_id: String,
    pub account_label: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCanvasCaptureOutput {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    runtime_state_object_key: Option<String>,
    #[serde(default)]
    browser_runtime_state_object_key: Option<String>,
    #[serde(default)]
    suggested_share_id: Option<String>,
    #[serde(default)]
    api_keys: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiBusinessCaptureOutput {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    jwt: Option<String>,
    #[serde(default)]
    config_id: Option<String>,
    #[serde(default)]
    session: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiWebCaptureOutput {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    auth_token: Option<String>,
    #[serde(default)]
    account_index: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    build_label: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    app_page_path: Option<String>,
    #[serde(default)]
    endpoint_path: Option<String>,
    #[serde(default)]
    referer: Option<String>,
    #[serde(default)]
    model_headers: Option<HashMap<String, String>>,
    #[serde(default)]
    request_context_header: Option<String>,
    #[serde(default)]
    response_status: Option<u16>,
    #[serde(default)]
    response_contains_paris: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteGeminiAuthInvocationResponse {
    ok: bool,
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<RemoteGeminiAuthInvocationError>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteGeminiAuthInvocationError {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteGeminiAuthInvocationRequest<'a> {
    provider: &'a str,
    provider_account_id: &'a str,
    endpoint_kind: &'a str,
    execution_mode: &'a str,
    input: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeminiAuthHelperExecutionMode {
    RemotePreferred,
    LocalOnly,
    HostExecutorRequired,
}

#[derive(Clone, Debug)]
struct GeminiAuthSessionControlState {
    complete_signal_relative_path: String,
}

#[derive(Default)]
pub struct GeminiAuthSessionManager {
    sessions: DashMap<String, GeminiAuthSessionView>,
    controls: DashMap<String, GeminiAuthSessionControlState>,
}

static GEMINI_AUTH_SESSION_MANAGER: OnceLock<Arc<GeminiAuthSessionManager>> = OnceLock::new();

pub fn gemini_auth_session_manager() -> Arc<GeminiAuthSessionManager> {
    Arc::clone(
        GEMINI_AUTH_SESSION_MANAGER.get_or_init(|| Arc::new(GeminiAuthSessionManager::default())),
    )
}

impl GeminiAuthSessionManager {
    pub fn create_session(
        self: &Arc<Self>,
        input: CreateGeminiAuthSessionInput,
    ) -> GeminiAuthSessionView {
        let now = now_rfc3339();
        let session_id = Uuid::new_v4().to_string();
        let trimmed_provider_id = input.provider_id.trim().to_string();
        let mut session = GeminiAuthSessionView {
            id: session_id.clone(),
            target_family: input.target_family,
            provider_id: trimmed_provider_id.clone(),
            status: GeminiAuthSessionStatus::Pending,
            message: "Starting Gemini auth helper.".to_string(),
            created_at: now.clone(),
            updated_at: now,
            generated_drafts: Vec::new(),
        };

        match input.target_family {
            GeminiAuthFamily::GeminiBusiness => {
                session.status = GeminiAuthSessionStatus::WaitingUser;
                session.message = "Complete Gemini Business login, then trigger one Gemini Business request in the opened browser window.".to_string();
                session.updated_at = now_rfc3339();
                self.sessions.insert(session_id.clone(), session.clone());
                let manager = Arc::clone(self);
                let provider_id = trimmed_provider_id.clone();
                tokio::spawn(async move {
                    manager
                        .run_business_capture_session(session_id, provider_id)
                        .await;
                });
                session
            }
            GeminiAuthFamily::GeminiWeb => {
                session.status = GeminiAuthSessionStatus::WaitingUser;
                session.message = "Complete Gemini Web login for /u/1/, then send the exact validation prompt in the opened browser window.".to_string();
                session.updated_at = now_rfc3339();
                self.sessions.insert(session_id.clone(), session.clone());
                let manager = Arc::clone(self);
                let provider_id = trimmed_provider_id.clone();
                tokio::spawn(async move {
                    manager
                        .run_web_capture_session(session_id, provider_id)
                        .await;
                });
                session
            }
            GeminiAuthFamily::GeminiCanvas | GeminiAuthFamily::GeminiCanvasChat => {
                let control_state = match prepare_gemini_auth_session_control_state(&session_id) {
                    Ok(control_state) => control_state,
                    Err(error) => {
                        session.status = GeminiAuthSessionStatus::Failed;
                        session.message = error;
                        session.updated_at = now_rfc3339();
                        self.sessions.insert(session_id.clone(), session.clone());
                        return session;
                    }
                };
                session.status = GeminiAuthSessionStatus::WaitingUser;
                session.message = "Complete Gemini login in the opened browser window.".to_string();
                session.updated_at = now_rfc3339();
                self.sessions.insert(session_id.clone(), session.clone());
                self.controls.insert(session_id.clone(), control_state);
                let manager = Arc::clone(self);
                let provider_id = trimmed_provider_id.clone();
                tokio::spawn(async move {
                    manager
                        .run_canvas_capture_session(
                            session_id,
                            provider_id,
                            input.target_family,
                            input.account_label,
                        )
                        .await;
                });
                session
            }
        }
    }

    async fn run_business_capture_session(
        self: Arc<Self>,
        session_id: String,
        provider_id: String,
    ) {
        match run_gemini_business_capture_worker(provider_id.as_str()).await {
            Ok(output) => {
                let generated_drafts = build_business_generated_drafts(
                    output
                        .jwt
                        .as_deref()
                        .expect("validated Gemini Business jwt"),
                    output
                        .config_id
                        .as_deref()
                        .expect("validated Gemini Business configId"),
                    output
                        .session
                        .as_deref()
                        .expect("validated Gemini Business session"),
                );
                self.update_session(
                    session_id.as_str(),
                    GeminiAuthSessionStatus::Succeeded,
                    "Gemini Business runtime captured.".to_string(),
                    generated_drafts,
                );
            }
            Err(message) => {
                self.update_session(
                    session_id.as_str(),
                    GeminiAuthSessionStatus::Failed,
                    message,
                    Vec::new(),
                );
            }
        }
    }

    async fn run_web_capture_session(self: Arc<Self>, session_id: String, provider_id: String) {
        match run_gemini_web_capture_worker(provider_id.as_str()).await {
            Ok(output) => {
                let generated_drafts = build_web_generated_drafts(
                    provider_id.as_str(),
                    output
                        .api_key
                        .as_deref()
                        .expect("validated Gemini Web primary cookie"),
                    output.auth_token.as_deref(),
                    &output,
                );
                self.update_session(
                    session_id.as_str(),
                    GeminiAuthSessionStatus::Succeeded,
                    "Gemini Web /u/1/ runtime captured.".to_string(),
                    generated_drafts,
                );
            }
            Err(message) => {
                self.update_session(
                    session_id.as_str(),
                    GeminiAuthSessionStatus::Failed,
                    message,
                    Vec::new(),
                );
            }
        }
    }

    pub fn get_session(&self, session_id: &str) -> Option<GeminiAuthSessionView> {
        self.sessions
            .get(session_id)
            .map(|entry| entry.value().clone())
    }

    pub fn request_manual_completion(
        &self,
        session_id: &str,
    ) -> Result<GeminiAuthSessionView, String> {
        let normalized_session_id = session_id.trim();
        let session = self
            .sessions
            .get(normalized_session_id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| {
                format!("Gateway Gemini auth session '{normalized_session_id}' was not found.")
            })?;
        if session.status != GeminiAuthSessionStatus::WaitingUser {
            return Ok(session);
        }
        if matches!(
            session.target_family,
            GeminiAuthFamily::GeminiBusiness | GeminiAuthFamily::GeminiWeb
        ) {
            return Err("This Gemini auth session requires one captured request in the opened browser window before it can complete.".to_string());
        }
        let control_state = self
            .controls
            .get(normalized_session_id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| {
                format!(
                    "Gateway Gemini auth session '{normalized_session_id}' does not have a manual-completion control file."
                )
            })?;
        let complete_signal_path =
            absolute_control_path(control_state.complete_signal_relative_path.as_str());
        if let Some(parent) = complete_signal_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to prepare the Gemini manual-completion control directory at {}: {error}",
                    parent.display()
                )
            })?;
        }
        std::fs::write(&complete_signal_path, b"complete\n").map_err(|error| {
            format!(
                "Failed to write the Gemini manual-completion signal file at {}: {error}",
                complete_signal_path.display()
            )
        })?;
        self.update_session(
            normalized_session_id,
            GeminiAuthSessionStatus::WaitingUser,
            "Manual Gemini import requested. Finishing capture. If Google opened an account verification challenge, complete that challenge in the helper browser first.".to_string(),
            Vec::new(),
        );
        self.get_session(normalized_session_id).ok_or_else(|| {
            format!("Gateway Gemini auth session '{normalized_session_id}' disappeared.")
        })
    }

    async fn run_canvas_capture_session(
        self: Arc<Self>,
        session_id: String,
        provider_id: String,
        target_family: GeminiAuthFamily,
        account_label: Option<String>,
    ) {
        let force_complete_signal_relative_path = self
            .controls
            .get(session_id.as_str())
            .map(|entry| entry.value().complete_signal_relative_path.clone());
        match run_gemini_canvas_capture_worker(
            provider_id.as_str(),
            target_family,
            account_label,
            force_complete_signal_relative_path.as_deref(),
        )
        .await
        {
            Ok(output) => {
                let runtime_state_object_key = output
                    .runtime_state_object_key
                    .as_deref()
                    .expect("validated Gemini Canvas runtimeStateObjectKey");
                let generated_drafts = build_canvas_generated_drafts(
                    runtime_state_object_key,
                    output
                        .suggested_share_id
                        .as_deref()
                        .unwrap_or(DEFAULT_GEMINI_CANVAS_SHARE_ID),
                    output.browser_runtime_state_object_key.as_deref(),
                    output.api_keys.as_deref(),
                );
                self.update_session(
                    session_id.as_str(),
                    GeminiAuthSessionStatus::Succeeded,
                    "Gemini Canvas runtime captured.".to_string(),
                    generated_drafts,
                );
                self.cleanup_control_state(session_id.as_str());
            }
            Err(message) => {
                self.update_session(
                    session_id.as_str(),
                    GeminiAuthSessionStatus::Failed,
                    message,
                    Vec::new(),
                );
                self.cleanup_control_state(session_id.as_str());
            }
        }
    }

    fn update_session(
        &self,
        session_id: &str,
        status: GeminiAuthSessionStatus,
        message: String,
        generated_drafts: Vec<GeminiGeneratedCredentialDraft>,
    ) {
        if let Some(mut entry) = self.sessions.get_mut(session_id) {
            entry.status = status;
            entry.message = message;
            entry.updated_at = now_rfc3339();
            entry.generated_drafts = generated_drafts;
        }
    }

    fn cleanup_control_state(&self, session_id: &str) {
        if let Some((_, control_state)) = self.controls.remove(session_id) {
            let complete_signal_path =
                absolute_control_path(control_state.complete_signal_relative_path.as_str());
            let _ = std::fs::remove_file(&complete_signal_path);
            if let Some(parent) = complete_signal_path.parent() {
                let _ = std::fs::remove_dir(parent);
            }
        }
    }
}
fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
#[path = "gemini_auth_session_tests.rs"]
mod tests;
