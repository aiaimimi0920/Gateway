use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use dashmap::DashMap;
use rquest::{Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::process::Command;
use tracing::debug;
use uuid::Uuid;

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

async fn run_gemini_canvas_capture_worker(
    provider_id: &str,
    target_family: GeminiAuthFamily,
    account_label: Option<String>,
    force_complete_signal_relative_path: Option<&str>,
) -> Result<GeminiCanvasCaptureOutput, String> {
    match current_gemini_auth_helper_execution_mode() {
        GeminiAuthHelperExecutionMode::RemotePreferred => {
            match run_remote_gemini_auth_capture_worker(
                provider_id,
                target_family,
                account_label.clone(),
                force_complete_signal_relative_path,
            )
            .await
            .and_then(parse_remote_gemini_canvas_capture_output)
            {
                Ok(output) => return Ok(output),
                Err(error) => {
                    if !local_headed_browser_launch_supported() {
                        return Err(error);
                    }
                    debug!(
                        provider_id,
                        ?target_family,
                        error = %error,
                        "remote Gemini auth helper failed; falling back to local browser launch"
                    );
                }
            }
        }
        GeminiAuthHelperExecutionMode::HostExecutorRequired => {
            return Err(host_executor_required_message(
                "Gemini Canvas",
                gemini_auth_remote_executor_base_url().as_deref(),
            ));
        }
        GeminiAuthHelperExecutionMode::LocalOnly => {}
    }

    let script_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join(GEMINI_CANVAS_CAPTURE_SCRIPT);
    let node_bin = std::env::var("GEMINI_CANVAS_CAPTURE_NODE_BIN")
        .ok()
        .or_else(|| std::env::var("GEMINI_CANVAS_BROWSER_NODE_BIN").ok())
        .unwrap_or_else(|| "node".to_string());
    let mut command = Command::new(node_bin);
    command
        .arg(&script_path)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("GEMINI_CANVAS_CAPTURE_OUTPUT_MODE", "storage_state")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(label) = account_label
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        command.env("GEMINI_CANVAS_CAPTURE_ACCOUNT_LABEL", label);
    }
    if let Some(relative_path) = force_complete_signal_relative_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        command.env("GEMINI_CANVAS_CAPTURE_FORCE_COMPLETE_FILE", relative_path);
    }

    let output = tokio::time::timeout(
        Duration::from_secs(GEMINI_AUTH_HELPER_TIMEOUT_SECS),
        command.output(),
    )
    .await
    .map_err(|_| "Gemini Canvas auth helper timed out before producing output.".to_string())?
    .map_err(|error| {
        format!(
            "Failed to launch Gemini Canvas auth helper at {}: {error}",
            script_path.display()
        )
    })?;
    parse_gemini_canvas_capture_output(
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
        output.status.success(),
    )
}

async fn run_gemini_business_capture_worker(
    provider_id: &str,
) -> Result<GeminiBusinessCaptureOutput, String> {
    match current_gemini_auth_helper_execution_mode() {
        GeminiAuthHelperExecutionMode::RemotePreferred => {
            match run_remote_gemini_auth_capture_worker(
                provider_id,
                GeminiAuthFamily::GeminiBusiness,
                None,
                None,
            )
            .await
            .and_then(parse_remote_gemini_business_capture_output)
            {
                Ok(output) => return Ok(output),
                Err(error) => {
                    if !local_headed_browser_launch_supported() {
                        return Err(error);
                    }
                    debug!(
                        provider_id,
                        error = %error,
                        "remote Gemini Business auth helper failed; falling back to local browser launch"
                    );
                }
            }
        }
        GeminiAuthHelperExecutionMode::HostExecutorRequired => {
            return Err(host_executor_required_message(
                "Gemini Business",
                gemini_auth_remote_executor_base_url().as_deref(),
            ));
        }
        GeminiAuthHelperExecutionMode::LocalOnly => {}
    }

    let script_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join(GEMINI_BUSINESS_CAPTURE_SCRIPT);
    let node_bin = std::env::var("GEMINI_BUSINESS_CAPTURE_NODE_BIN")
        .ok()
        .or_else(|| std::env::var("GEMINI_CANVAS_BROWSER_NODE_BIN").ok())
        .unwrap_or_else(|| "node".to_string());
    let output = tokio::time::timeout(
        Duration::from_secs(GEMINI_AUTH_HELPER_TIMEOUT_SECS),
        Command::new(node_bin)
            .arg(&script_path)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| "Gemini Business auth helper timed out before producing output.".to_string())?
    .map_err(|error| {
        format!(
            "Failed to launch Gemini Business auth helper at {}: {error}",
            script_path.display()
        )
    })?;
    parse_gemini_business_capture_output(
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
        output.status.success(),
    )
}

async fn run_gemini_web_capture_worker(
    provider_id: &str,
) -> Result<GeminiWebCaptureOutput, String> {
    match current_gemini_auth_helper_execution_mode() {
        GeminiAuthHelperExecutionMode::RemotePreferred => {
            match run_remote_gemini_auth_capture_worker(
                provider_id,
                GeminiAuthFamily::GeminiWeb,
                None,
                None,
            )
            .await
            .and_then(parse_remote_gemini_web_capture_output)
            {
                Ok(output) => return Ok(output),
                Err(error) => {
                    if !local_headed_browser_launch_supported() {
                        return Err(error);
                    }
                    debug!(
                        provider_id,
                        error = %error,
                        "remote Gemini Web auth helper failed; falling back to local browser launch"
                    );
                }
            }
        }
        GeminiAuthHelperExecutionMode::HostExecutorRequired => {
            return Err(host_executor_required_message(
                "Gemini Web",
                gemini_auth_remote_executor_base_url().as_deref(),
            ));
        }
        GeminiAuthHelperExecutionMode::LocalOnly => {}
    }

    let script_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join(GEMINI_WEB_CAPTURE_SCRIPT);
    let node_bin = std::env::var("GEMINI_WEB_CAPTURE_NODE_BIN")
        .ok()
        .or_else(|| std::env::var("GEMINI_CANVAS_BROWSER_NODE_BIN").ok())
        .unwrap_or_else(|| "node".to_string());
    let output = tokio::time::timeout(
        Duration::from_secs(GEMINI_AUTH_HELPER_TIMEOUT_SECS),
        Command::new(node_bin)
            .arg(&script_path)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| "Gemini Web auth helper timed out before producing output.".to_string())?
    .map_err(|error| {
        format!(
            "Failed to launch Gemini Web auth helper at {}: {error}",
            script_path.display()
        )
    })?;
    parse_gemini_web_capture_output(
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
        output.status.success(),
    )
}

async fn run_remote_gemini_auth_capture_worker(
    provider_id: &str,
    target_family: GeminiAuthFamily,
    account_label: Option<String>,
    force_complete_signal_relative_path: Option<&str>,
) -> Result<Value, String> {
    let base_url = gemini_auth_remote_executor_base_url()
        .ok_or_else(|| "Gemini host browser executor is not configured.".to_string())?;
    let client = Client::builder()
        .timeout(Duration::from_secs(GEMINI_AUTH_REMOTE_HELPER_TIMEOUT_SECS))
        .build()
        .map_err(|error| {
            format!("Failed to build the Gemini host browser executor client: {error}")
        })?;
    let mut input = json!({
        "targetFamily": target_family,
    });
    if let Some(label) = account_label
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        input["accountLabel"] = Value::String(label);
    }
    if let Some(relative_path) = force_complete_signal_relative_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        input["forceCompleteSignalRelativePath"] = Value::String(relative_path.to_string());
    }
    let request = RemoteGeminiAuthInvocationRequest {
        provider: GEMINI_AUTH_REMOTE_PROVIDER,
        provider_account_id: provider_id,
        endpoint_kind: GEMINI_AUTH_REMOTE_ENDPOINT_KIND,
        execution_mode: GEMINI_AUTH_REMOTE_EXECUTION_MODE,
        input,
    };
    let url = format!("{base_url}/v1/internal/browser-executor/execute");
    let mut builder = client
        .request(Method::POST, &url)
        .header(rquest::header::CONTENT_TYPE, "application/json")
        .json(&request);
    if let Some(token) = gemini_auth_remote_executor_bearer_token() {
        builder = builder.bearer_auth(token);
    }

    let response = builder.send().await.map_err(|error| {
        format!(
            "Failed to reach the Gemini host browser executor at {url}: {error}. {}",
            GEMINI_AUTH_HOST_HELPER_START_HINT
        )
    })?;
    let status = response.status().as_u16();
    let body_text = response.text().await.unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(format!(
            "Gemini host browser executor at {url} returned status {status}: {}",
            summarize_output(&body_text)
        ));
    }

    let parsed = serde_json::from_str::<RemoteGeminiAuthInvocationResponse>(&body_text).map_err(
        |error| {
            format!(
                "Gemini host browser executor at {url} returned invalid JSON: {error}. payload: {}",
                summarize_output(&body_text)
            )
        },
    )?;
    if parsed.ok {
        return parsed.result.ok_or_else(|| {
            "Gemini host browser executor succeeded but returned no result payload.".to_string()
        });
    }

    Err(format_remote_gemini_auth_invocation_error(
        parsed.status,
        parsed.error,
    ))
}

fn parse_remote_gemini_canvas_capture_output(
    value: Value,
) -> Result<GeminiCanvasCaptureOutput, String> {
    let parsed = serde_json::from_value::<GeminiCanvasCaptureOutput>(value).map_err(|error| {
        format!("Gemini host browser executor returned an invalid Gemini Canvas payload: {error}")
    })?;
    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| "Gemini host browser executor failed.".to_string()));
    }
    let runtime_state_object_key = parsed
        .runtime_state_object_key
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| {
            "Gemini host browser executor did not return runtimeStateObjectKey.".to_string()
        })?;
    let _ = runtime_state_object_key;
    Ok(parsed)
}

fn parse_remote_gemini_business_capture_output(
    value: Value,
) -> Result<GeminiBusinessCaptureOutput, String> {
    let parsed = serde_json::from_value::<GeminiBusinessCaptureOutput>(value).map_err(|error| {
        format!("Gemini host browser executor returned an invalid Gemini Business payload: {error}")
    })?;
    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| "Gemini host browser executor failed.".to_string()));
    }
    let jwt = parsed
        .jwt
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| "Gemini host browser executor did not return jwt.".to_string())?;
    let config_id = parsed
        .config_id
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| "Gemini host browser executor did not return configId.".to_string())?;
    let session = parsed
        .session
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| "Gemini host browser executor did not return session.".to_string())?;
    let _ = (jwt, config_id, session);
    Ok(parsed)
}

fn parse_remote_gemini_web_capture_output(value: Value) -> Result<GeminiWebCaptureOutput, String> {
    let parsed = serde_json::from_value::<GeminiWebCaptureOutput>(value).map_err(|error| {
        format!("Gemini host browser executor returned an invalid Gemini Web payload: {error}")
    })?;
    validate_gemini_web_capture_output(parsed, "Gemini host browser executor")
}

fn gemini_auth_remote_executor_base_url() -> Option<String> {
    first_non_empty_env_var(&[
        "GATEWAY_GEMINI_AUTH_EXECUTOR_BASE_URL",
        "GATEWAY_BROWSER_EXECUTOR_BASE_URL",
    ])
    .map(|value| value.trim_end_matches('/').to_string())
}

fn gemini_auth_remote_executor_bearer_token() -> Option<String> {
    first_non_empty_env_var(&[
        "GATEWAY_GEMINI_AUTH_EXECUTOR_BEARER_TOKEN",
        "GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN",
    ])
}

fn prepare_gemini_auth_session_control_state(
    session_id: &str,
) -> Result<GeminiAuthSessionControlState, String> {
    let control_dir = absolute_control_path(&format!("{GEMINI_AUTH_CONTROL_ROOT}/{session_id}"));
    std::fs::create_dir_all(&control_dir).map_err(|error| {
        format!(
            "Failed to create Gemini auth control directory at {}: {error}",
            control_dir.display()
        )
    })?;
    let complete_signal_relative_path =
        format!("{GEMINI_AUTH_CONTROL_ROOT}/{session_id}/complete.signal");
    let complete_signal_absolute_path =
        absolute_control_path(complete_signal_relative_path.as_str());
    let _ = std::fs::remove_file(complete_signal_absolute_path);
    Ok(GeminiAuthSessionControlState {
        complete_signal_relative_path,
    })
}

fn absolute_control_path(relative_path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path)
}

fn first_non_empty_env_var(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn current_gemini_auth_helper_execution_mode() -> GeminiAuthHelperExecutionMode {
    resolve_gemini_auth_helper_execution_mode(
        std::env::consts::OS,
        std::env::var("DISPLAY").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        gemini_auth_remote_executor_base_url().as_deref(),
    )
}

fn resolve_gemini_auth_helper_execution_mode(
    platform: &str,
    display: Option<&str>,
    wayland_display: Option<&str>,
    remote_base_url: Option<&str>,
) -> GeminiAuthHelperExecutionMode {
    if let Some(base_url) = remote_base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let _ = base_url;
        return GeminiAuthHelperExecutionMode::RemotePreferred;
    }
    if local_headed_browser_launch_supported_for(platform, display, wayland_display) {
        GeminiAuthHelperExecutionMode::LocalOnly
    } else {
        GeminiAuthHelperExecutionMode::HostExecutorRequired
    }
}

fn local_headed_browser_launch_supported() -> bool {
    local_headed_browser_launch_supported_for(
        std::env::consts::OS,
        std::env::var("DISPLAY").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
    )
}

fn local_headed_browser_launch_supported_for(
    platform: &str,
    display: Option<&str>,
    wayland_display: Option<&str>,
) -> bool {
    if platform == "windows" {
        return true;
    }
    display
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        || wayland_display
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
}

fn host_executor_required_message(surface_label: &str, remote_base_url: Option<&str>) -> String {
    if let Some(base_url) = remote_base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!(
            "{surface_label} manual add could not use the configured host browser executor at {base_url}. {}",
            GEMINI_AUTH_HOST_HELPER_START_HINT
        );
    }
    format!(
        "{surface_label} manual add requires a visible host browser in this Docker-style workflow. Configure GATEWAY_GEMINI_AUTH_EXECUTOR_BASE_URL (for example http://host.docker.internal:42341) and {}",
        GEMINI_AUTH_HOST_HELPER_START_HINT
    )
}

fn format_remote_gemini_auth_invocation_error(
    top_level_status: Option<u16>,
    error: Option<RemoteGeminiAuthInvocationError>,
) -> String {
    let Some(error) = error else {
        return format!(
            "Gemini host browser executor failed without an error payload.{}",
            top_level_status
                .map(|status| format!(" status={status}"))
                .unwrap_or_default()
        );
    };
    let mut message = error
        .message
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Gemini host browser executor failed.")
        .to_string();
    if let Some(code) = error
        .code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        message.push_str(&format!(" [code={code}]"));
    }
    if let Some(status) = error.status.or(top_level_status) {
        message.push_str(&format!(" (status {status})"));
    }
    if let Some(body) = error
        .body
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        message.push_str(&format!(" body: {}", summarize_output(body)));
    }
    message
}

fn parse_gemini_canvas_capture_output(
    stdout: &str,
    stderr: &str,
    exited_successfully: bool,
) -> Result<GeminiCanvasCaptureOutput, String> {
    let combined = if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        format!("{}\n{}", stdout.trim(), stderr.trim())
    };
    let json_text = extract_last_json_object(&combined).ok_or_else(|| {
        format!(
            "Gemini Canvas auth helper returned no parseable JSON output. stdout: {} stderr: {}",
            summarize_output(stdout),
            summarize_output(stderr)
        )
    })?;
    let parsed =
        serde_json::from_str::<GeminiCanvasCaptureOutput>(&json_text).map_err(|error| {
            format!(
                "Failed to parse Gemini Canvas auth helper JSON output: {error}. payload: {}",
                summarize_output(&json_text)
            )
        })?;
    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| "Gemini Canvas auth helper failed.".to_string()));
    }
    let runtime_state_object_key = parsed
        .runtime_state_object_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "Gemini Canvas auth helper did not return runtimeStateObjectKey.".to_string()
        })?;
    if !exited_successfully {
        return Err(format!(
            "Gemini Canvas auth helper exited before completing successfully but returned runtimeStateObjectKey={runtime_state_object_key}."
        ));
    }
    Ok(parsed)
}

fn build_canvas_generated_drafts(
    runtime_state_object_key: &str,
    suggested_share_id: &str,
    browser_runtime_state_object_key: Option<&str>,
    captured_api_keys: Option<&[String]>,
) -> Vec<GeminiGeneratedCredentialDraft> {
    let suffix = OffsetDateTime::now_utc().unix_timestamp().to_string();
    let normalized_api_keys = captured_api_keys
        .unwrap_or(&[])
        .iter()
        .map(|candidate| candidate.trim())
        .filter(|candidate| !candidate.is_empty())
        .fold(Vec::<String>::new(), |mut keys, candidate| {
            if !keys.iter().any(|existing| existing == candidate) {
                keys.push(candidate.to_string());
            }
            keys
        });
    let mut canvas_extra_body = json!({
        "shareId": suggested_share_id,
    });
    if let Some(browser_runtime_state_object_key) = browser_runtime_state_object_key
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
    {
        canvas_extra_body["browserRuntimeStateObjectKey"] = json!(browser_runtime_state_object_key);
    }
    let mut canvas_chat_extra_body = json!({
        "shareId": suggested_share_id,
        "apiBaseUrl": GEMINI_CANVAS_CHAT_API_BASE_URL,
    });
    if let Some(browser_runtime_state_object_key) = canvas_extra_body
        .get("browserRuntimeStateObjectKey")
        .cloned()
    {
        canvas_chat_extra_body["browserRuntimeStateObjectKey"] = browser_runtime_state_object_key;
    }
    if !normalized_api_keys.is_empty() {
        canvas_chat_extra_body["apiKeys"] = json!(normalized_api_keys);
    }
    vec![
        GeminiGeneratedCredentialDraft {
            provider_id: "gemini-canvas".to_string(),
            credential: json!({
                "id": format!("gemini-canvas-manual-{suffix}"),
                "account_name": format!("Gemini Canvas Manual {suffix}"),
                "runtime_state_object_key": runtime_state_object_key,
                "extra_body": canvas_extra_body,
            }),
            secret_edits: Vec::new(),
        },
        GeminiGeneratedCredentialDraft {
            provider_id: "gemini-canvas-chat".to_string(),
            credential: json!({
                "id": format!("gemini-canvas-chat-manual-{suffix}"),
                "account_name": format!("Gemini Canvas Chat Manual {suffix}"),
                "runtime_state_object_key": runtime_state_object_key,
                "extra_body": canvas_chat_extra_body,
            }),
            secret_edits: Vec::new(),
        },
    ]
}

fn build_business_generated_drafts(
    jwt: &str,
    config_id: &str,
    session_name: &str,
) -> Vec<GeminiGeneratedCredentialDraft> {
    let suffix = OffsetDateTime::now_utc().unix_timestamp().to_string();
    vec![GeminiGeneratedCredentialDraft {
        provider_id: "gemini-business".to_string(),
        credential: json!({
            "id": format!("gemini-business-manual-{suffix}"),
            "account_name": format!("Gemini Business Manual {suffix}"),
            "api_key": "",
            "extra_body": {
                "configId": config_id,
                "session": session_name,
            },
        }),
        secret_edits: vec![GeminiAuthSecretEdit {
            field: "api_key".to_string(),
            operation: "replace".to_string(),
            value: jwt.to_string(),
        }],
    }]
}

fn build_web_generated_drafts(
    provider_id: &str,
    api_key: &str,
    auth_token: Option<&str>,
    output: &GeminiWebCaptureOutput,
) -> Vec<GeminiGeneratedCredentialDraft> {
    let suffix = OffsetDateTime::now_utc().unix_timestamp().to_string();
    let mut extra_body = serde_json::Map::new();
    for (key, value) in [
        ("authUser", output.account_index.as_deref()),
        ("accessToken", output.access_token.as_deref()),
        ("buildLabel", output.build_label.as_deref()),
        ("sessionId", output.session_id.as_deref()),
        ("language", output.language.as_deref()),
        ("appPagePath", output.app_page_path.as_deref()),
        ("endpointPath", output.endpoint_path.as_deref()),
        (
            "requestContextHeader",
            output.request_context_header.as_deref(),
        ),
    ] {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            extra_body.insert(key.to_string(), Value::String(value.to_string()));
        }
    }
    if let Some(model_headers) = output.model_headers.as_ref() {
        for (header_name, extra_body_key) in [
            ("x-goog-ext-525001261-jspb", "modelHeader"),
            ("x-goog-ext-73010989-jspb", "modelHeader2"),
            ("x-goog-ext-73010990-jspb", "modelHeader3"),
        ] {
            if let Some(value) = model_headers
                .get(header_name)
                .map(String::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                extra_body.insert(extra_body_key.to_string(), Value::String(value.to_string()));
            }
        }
    }

    let mut headers = serde_json::Map::new();
    if let Some(referer) = output
        .referer
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        headers.insert("Referer".to_string(), Value::String(referer.to_string()));
    }
    if let Some(account_index) = output
        .account_index
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        headers.insert(
            "X-Goog-AuthUser".to_string(),
            Value::String(account_index.to_string()),
        );
    }

    let mut secret_edits = vec![GeminiAuthSecretEdit {
        field: "api_key".to_string(),
        operation: "replace".to_string(),
        value: api_key.to_string(),
    }];
    if let Some(auth_token) = auth_token.map(str::trim).filter(|value| !value.is_empty()) {
        secret_edits.push(GeminiAuthSecretEdit {
            field: "auth_token".to_string(),
            operation: "replace".to_string(),
            value: auth_token.to_string(),
        });
    }

    vec![GeminiGeneratedCredentialDraft {
        provider_id: provider_id.to_string(),
        credential: json!({
            "id": format!("gemini-web-manual-{suffix}"),
            "account_name": format!("Gemini Web /u/1/ Manual {suffix}"),
            "api_key": "",
            "auth_token": "",
            "headers": headers,
            "extra_body": extra_body,
        }),
        secret_edits,
    }]
}

fn parse_gemini_business_capture_output(
    stdout: &str,
    stderr: &str,
    exited_successfully: bool,
) -> Result<GeminiBusinessCaptureOutput, String> {
    let combined = if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        format!("{}\n{}", stdout.trim(), stderr.trim())
    };
    let json_text = extract_last_json_object(&combined).ok_or_else(|| {
        format!(
            "Gemini Business auth helper returned no parseable JSON output. stdout: {} stderr: {}",
            summarize_output(stdout),
            summarize_output(stderr)
        )
    })?;
    let parsed =
        serde_json::from_str::<GeminiBusinessCaptureOutput>(&json_text).map_err(|error| {
            format!(
                "Failed to parse Gemini Business auth helper JSON output: {error}. payload: {}",
                summarize_output(&json_text)
            )
        })?;
    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| "Gemini Business auth helper failed.".to_string()));
    }
    let jwt = parsed
        .jwt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Gemini Business auth helper did not return jwt.".to_string())?;
    let config_id = parsed
        .config_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Gemini Business auth helper did not return configId.".to_string())?;
    let session = parsed
        .session
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Gemini Business auth helper did not return session.".to_string())?;
    if !exited_successfully {
        return Err(format!(
            "Gemini Business auth helper exited before completing successfully but returned jwt/configId/session. jwt_len={} configId={} session={}",
            jwt.len(),
            config_id,
            session
        ));
    }
    Ok(parsed)
}

fn parse_gemini_web_capture_output(
    stdout: &str,
    stderr: &str,
    exited_successfully: bool,
) -> Result<GeminiWebCaptureOutput, String> {
    let combined = if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        format!("{}\n{}", stdout.trim(), stderr.trim())
    };
    let json_text = extract_last_json_object(&combined).ok_or_else(|| {
        format!(
            "Gemini Web auth helper returned no parseable JSON output. stdout: {} stderr: {}",
            summarize_output(stdout),
            summarize_output(stderr)
        )
    })?;
    let parsed = serde_json::from_str::<GeminiWebCaptureOutput>(&json_text).map_err(|error| {
        format!(
            "Failed to parse Gemini Web auth helper JSON output: {error}. payload: {}",
            summarize_output(&json_text)
        )
    })?;
    let parsed = validate_gemini_web_capture_output(parsed, "Gemini Web auth helper")?;
    if !exited_successfully {
        return Err(
            "Gemini Web auth helper exited before completing successfully after returning runtime material."
                .to_string(),
        );
    }
    Ok(parsed)
}

fn validate_gemini_web_capture_output(
    parsed: GeminiWebCaptureOutput,
    source: &str,
) -> Result<GeminiWebCaptureOutput, String> {
    if !parsed.ok {
        return Err(parsed
            .error
            .clone()
            .unwrap_or_else(|| format!("{source} failed.")));
    }
    parsed
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{source} did not return the primary Gemini Web cookie."))?;
    let response_status = parsed
        .response_status
        .ok_or_else(|| format!("{source} did not return the validation response status."))?;
    if !(200..300).contains(&response_status) {
        return Err(format!(
            "{source} captured the validation request, but Gemini Web returned HTTP {response_status}."
        ));
    }
    if parsed.response_contains_paris != Some(true) {
        return Err(format!(
            "{source} captured the validation request, but the response did not contain the expected answer."
        ));
    }
    Ok(parsed)
}

fn extract_last_json_object(text: &str) -> Option<String> {
    let starts = text
        .match_indices('{')
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    for start in starts.into_iter().rev() {
        let candidate = text[start..].trim();
        if serde_json::from_str::<Value>(candidate).is_ok() {
            return Some(candidate.to_string());
        }
    }
    None
}

fn summarize_output(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= 400 {
        return trimmed.to_string();
    }
    format!("{}...(truncated)", &trimmed[..400])
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        build_business_generated_drafts, build_canvas_generated_drafts, build_web_generated_drafts,
        extract_last_json_object, now_rfc3339, parse_remote_gemini_business_capture_output,
        parse_remote_gemini_canvas_capture_output, parse_remote_gemini_web_capture_output,
        prepare_gemini_auth_session_control_state, resolve_gemini_auth_helper_execution_mode,
        GeminiAuthFamily, GeminiAuthHelperExecutionMode, GeminiAuthSessionControlState,
        GeminiAuthSessionManager, GeminiAuthSessionStatus, GeminiAuthSessionView,
        GeminiWebCaptureOutput,
    };
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::sync::Arc;
    use time::OffsetDateTime;

    #[test]
    fn extracts_last_json_object_after_helper_logs() {
        let output = r#"
[gemini-canvas-export] Opening browser at https://gemini.google.com/app
[gemini-canvas-export] Complete Gemini login in the opened browser window.
{
  "ok": true,
  "runtimeStateObjectKey": "credential-runtime/gemini-canvas/demo/storage-state.json",
  "suggestedShareId": "fe24c455a570"
}
"#;
        let extracted = extract_last_json_object(output).expect("json");
        assert!(extracted.contains("\"runtimeStateObjectKey\""));
    }

    #[test]
    fn build_canvas_generated_drafts_shares_runtime_material_across_both_families() {
        let drafts = build_canvas_generated_drafts(
            "credential-runtime/gemini-canvas/demo/storage-state.json",
            "fe24c455a570",
            None,
            None,
        );
        assert_eq!(drafts.len(), 2);
        assert_eq!(drafts[0].provider_id, "gemini-canvas");
        assert_eq!(drafts[1].provider_id, "gemini-canvas-chat");
        assert_eq!(
            drafts[0].credential["runtime_state_object_key"],
            Value::String("credential-runtime/gemini-canvas/demo/storage-state.json".to_string())
        );
        assert_eq!(
            drafts[1].credential["extra_body"]["apiBaseUrl"],
            Value::String("https://generativelanguage.googleapis.com/v1beta".to_string())
        );
        assert!(drafts[0].credential.get("api_key").is_none());
        assert!(drafts[1].credential.get("api_key").is_none());
    }

    #[test]
    fn build_canvas_generated_drafts_keeps_official_api_keys_out_of_program_owned_draft() {
        let drafts = build_canvas_generated_drafts(
            "credential-runtime/gemini-canvas/demo/storage-state.json",
            "fe24c455a570",
            Some("credential-runtime/gemini-canvas/demo/browser-profile"),
            Some(&[
                "AIzaCapturedOne123456789012".to_string(),
                "AIzaCapturedTwo123456789012".to_string(),
                "AIzaCapturedOne123456789012".to_string(),
            ]),
        );

        assert!(drafts[0].credential["extra_body"].get("apiKeys").is_none());
        assert_eq!(
            drafts[1].credential["extra_body"]["apiKeys"],
            json!(["AIzaCapturedOne123456789012", "AIzaCapturedTwo123456789012"])
        );
        assert_eq!(
            drafts[0].credential["extra_body"]["browserRuntimeStateObjectKey"],
            Value::String("credential-runtime/gemini-canvas/demo/browser-profile".to_string())
        );
        assert_eq!(
            drafts[1].credential["extra_body"]["browserRuntimeStateObjectKey"],
            Value::String("credential-runtime/gemini-canvas/demo/browser-profile".to_string())
        );
    }

    #[test]
    fn build_canvas_generated_drafts_keeps_browser_profile_override_for_profile_dirs() {
        let drafts = build_canvas_generated_drafts(
            "credential-runtime/gemini-canvas/demo/browser-profile",
            "fe24c455a570",
            Some("credential-runtime/gemini-canvas/demo/browser-profile"),
            None,
        );

        assert_eq!(
            drafts[0].credential["extra_body"]["browserRuntimeStateObjectKey"],
            Value::String("credential-runtime/gemini-canvas/demo/browser-profile".to_string())
        );
        assert_eq!(
            drafts[1].credential["extra_body"]["browserRuntimeStateObjectKey"],
            Value::String("credential-runtime/gemini-canvas/demo/browser-profile".to_string())
        );
    }

    #[test]
    fn build_business_generated_draft_keeps_jwt_in_secret_edits() {
        let drafts =
            build_business_generated_drafts("ey.demo.jwt", "cfg-123", "projects/demo/sessions/abc");
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].provider_id, "gemini-business");
        assert_eq!(
            drafts[0].credential["extra_body"]["configId"],
            Value::String("cfg-123".to_string())
        );
        assert_eq!(
            drafts[0].credential["extra_body"]["session"],
            Value::String("projects/demo/sessions/abc".to_string())
        );
        assert_eq!(drafts[0].secret_edits.len(), 1);
        assert_eq!(drafts[0].secret_edits[0].operation, "replace");
        assert_eq!(drafts[0].secret_edits[0].value, "ey.demo.jwt");
    }

    #[test]
    fn build_web_generated_draft_keeps_both_cookies_in_secret_edits() {
        let output = GeminiWebCaptureOutput {
            ok: true,
            error: None,
            api_key: Some("primary-cookie".to_string()),
            auth_token: Some("secondary-cookie".to_string()),
            account_index: Some("1".to_string()),
            access_token: Some("access-1".to_string()),
            build_label: Some("build-1".to_string()),
            session_id: Some("session-1".to_string()),
            language: Some("zh-CN".to_string()),
            app_page_path: Some("/u/1/app".to_string()),
            endpoint_path: Some(
                "/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate".to_string(),
            ),
            referer: Some("https://gemini.google.com/u/1/app".to_string()),
            model_headers: Some(HashMap::from([(
                "x-goog-ext-525001261-jspb".to_string(),
                "model-header".to_string(),
            )])),
            request_context_header: Some("context-header".to_string()),
            response_status: Some(200),
            response_contains_paris: Some(true),
        };
        let drafts = build_web_generated_drafts(
            "gemini-web-secondary",
            "primary-cookie",
            Some("secondary-cookie"),
            &output,
        );

        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].provider_id, "gemini-web-secondary");
        assert_eq!(drafts[0].credential["extra_body"]["authUser"], "1");
        assert_eq!(
            drafts[0].credential["extra_body"]["appPagePath"],
            "/u/1/app"
        );
        assert_eq!(drafts[0].credential["headers"]["X-Goog-AuthUser"], "1");
        assert_eq!(drafts[0].secret_edits.len(), 2);
        assert_eq!(drafts[0].secret_edits[0].field, "api_key");
        assert_eq!(drafts[0].secret_edits[1].field, "auth_token");
        assert!(drafts[0].credential["api_key"].as_str().unwrap().is_empty());
        assert!(drafts[0].credential["auth_token"]
            .as_str()
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn create_business_session_starts_waiting_for_manual_capture() {
        let manager = Arc::new(GeminiAuthSessionManager::default());
        let session = manager.create_session(super::CreateGeminiAuthSessionInput {
            target_family: GeminiAuthFamily::GeminiBusiness,
            provider_id: "gemini-business".to_string(),
            account_label: None,
        });
        assert_eq!(session.target_family, GeminiAuthFamily::GeminiBusiness);
        assert_eq!(session.status, GeminiAuthSessionStatus::WaitingUser);
        assert!(session.message.contains("Gemini Business"));
        let loaded = manager.get_session(&session.id).expect("stored session");
        assert_eq!(loaded.status, GeminiAuthSessionStatus::WaitingUser);
    }

    #[test]
    fn remote_gemini_canvas_payload_parses_runtime_state_contract() {
        let parsed = parse_remote_gemini_canvas_capture_output(json!({
            "ok": true,
            "targetFamily": "gemini-canvas",
            "runtimeStateObjectKey": "credential-runtime/gemini-canvas/host-export/storage-state.json",
            "browserRuntimeStateObjectKey": "credential-runtime/gemini-canvas/host-export",
            "suggestedShareId": "fe24c455a570",
            "apiKeys": ["AIzaCapturedOne123456789012"],
        }))
        .expect("parse remote canvas payload");

        assert_eq!(
            parsed.runtime_state_object_key.as_deref(),
            Some("credential-runtime/gemini-canvas/host-export/storage-state.json")
        );
        assert_eq!(
            parsed.browser_runtime_state_object_key.as_deref(),
            Some("credential-runtime/gemini-canvas/host-export")
        );
        assert_eq!(parsed.suggested_share_id.as_deref(), Some("fe24c455a570"));
        assert_eq!(
            parsed.api_keys.as_deref(),
            Some(&["AIzaCapturedOne123456789012".to_string()][..])
        );
    }

    #[test]
    fn remote_gemini_business_payload_parses_secret_fields_contract() {
        let parsed = parse_remote_gemini_business_capture_output(json!({
            "ok": true,
            "targetFamily": "gemini-business",
            "jwt": "ey.demo.jwt",
            "configId": "cfg-123",
            "session": "projects/demo/sessions/abc",
        }))
        .expect("parse remote business payload");

        assert_eq!(parsed.jwt.as_deref(), Some("ey.demo.jwt"));
        assert_eq!(parsed.config_id.as_deref(), Some("cfg-123"));
        assert_eq!(
            parsed.session.as_deref(),
            Some("projects/demo/sessions/abc")
        );
    }

    #[test]
    fn remote_gemini_web_payload_requires_successful_validation_response() {
        let parsed = parse_remote_gemini_web_capture_output(json!({
            "ok": true,
            "targetFamily": "gemini-web",
            "apiKey": "primary-cookie",
            "authToken": "secondary-cookie",
            "accountIndex": "1",
            "appPagePath": "/u/1/app",
            "responseStatus": 200,
            "responseContainsParis": true,
        }))
        .expect("parse remote web payload");

        assert_eq!(parsed.api_key.as_deref(), Some("primary-cookie"));
        assert_eq!(parsed.auth_token.as_deref(), Some("secondary-cookie"));
        assert_eq!(parsed.account_index.as_deref(), Some("1"));

        let error = parse_remote_gemini_web_capture_output(json!({
            "ok": true,
            "apiKey": "primary-cookie",
            "responseStatus": 401,
        }))
        .expect_err("401 capture must fail");
        assert!(error.contains("HTTP 401"));

        let error = parse_remote_gemini_web_capture_output(json!({
            "ok": true,
            "apiKey": "primary-cookie",
            "responseStatus": 200,
            "responseContainsParis": false,
        }))
        .expect_err("an incorrect validation answer must fail");
        assert!(error.contains("expected answer"));
    }

    #[test]
    fn displayless_linux_requires_host_executor_when_remote_helper_missing() {
        assert_eq!(
            resolve_gemini_auth_helper_execution_mode("linux", None, None, None),
            GeminiAuthHelperExecutionMode::HostExecutorRequired
        );
    }

    #[test]
    fn displayless_linux_prefers_remote_helper_when_configured() {
        assert_eq!(
            resolve_gemini_auth_helper_execution_mode(
                "linux",
                None,
                None,
                Some("http://host.docker.internal:42341"),
            ),
            GeminiAuthHelperExecutionMode::RemotePreferred
        );
    }

    #[test]
    fn request_manual_completion_marks_waiting_canvas_session_and_creates_signal_file() {
        let manager = GeminiAuthSessionManager::default();
        let session_id = format!(
            "manual-complete-{}",
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        );
        let control_state =
            prepare_gemini_auth_session_control_state(session_id.as_str()).expect("control state");
        let complete_signal_relative_path = control_state.complete_signal_relative_path.clone();
        let complete_signal_absolute_path =
            super::absolute_control_path(complete_signal_relative_path.as_str());

        manager.sessions.insert(
            session_id.clone(),
            GeminiAuthSessionView {
                id: session_id.clone(),
                target_family: GeminiAuthFamily::GeminiCanvas,
                provider_id: "gemini-canvas".to_string(),
                status: GeminiAuthSessionStatus::WaitingUser,
                message: "waiting".to_string(),
                created_at: now_rfc3339(),
                updated_at: now_rfc3339(),
                generated_drafts: Vec::new(),
            },
        );
        manager.controls.insert(
            session_id.clone(),
            GeminiAuthSessionControlState {
                complete_signal_relative_path,
            },
        );

        let updated = manager
            .request_manual_completion(session_id.as_str())
            .expect("manual completion");

        assert_eq!(updated.status, GeminiAuthSessionStatus::WaitingUser);
        assert!(updated.message.contains("Finishing capture"));
        assert!(complete_signal_absolute_path.exists());

        let _ = std::fs::remove_file(complete_signal_absolute_path);
    }
}
