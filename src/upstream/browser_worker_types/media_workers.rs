//! Media-worker stdin and stdout contracts, including provider-specific fields.

use serde_json::Value;

use crate::protocol::suno;

#[derive(Debug, serde::Serialize)]
pub(crate) struct LumalabsBrowserWorkerInput<'a> {
    #[serde(rename = "baseUrl")]
    pub(crate) base_url: &'a str,
    #[serde(rename = "realmId")]
    pub(crate) realm_id: &'a str,
    #[serde(rename = "mediaOperation", skip_serializing_if = "Option::is_none")]
    pub(crate) media_operation: Option<&'a str>,
    #[serde(rename = "artifactField")]
    pub(crate) artifact_field: &'a str,
    #[serde(rename = "sessionToken")]
    pub(crate) session_token: &'a str,
    #[serde(rename = "actionBody")]
    pub(crate) action_body: &'a Value,
    #[serde(rename = "autoDiscoverActionType")]
    pub(crate) auto_discover_action_type: bool,
    pub(crate) locale: &'a str,
    #[serde(rename = "timeoutMs")]
    pub(crate) timeout_ms: u64,
    #[serde(
        rename = "browserExecutablePath",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) browser_executable_path: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct LumalabsBrowserWorkerResult {
    pub(crate) ok: bool,
    #[serde(rename = "signedUrl")]
    pub(crate) signed_url: Option<String>,
    pub(crate) error: Option<LumalabsBrowserWorkerError>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct LumalabsBrowserWorkerError {
    pub(crate) code: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) status: Option<u16>,
    pub(crate) body: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct ProducerBrowserWorkerInput<'a> {
    #[serde(rename = "baseUrl")]
    pub(crate) base_url: &'a str,
    #[serde(rename = "authToken", skip_serializing_if = "Option::is_none")]
    pub(crate) auth_token: Option<String>,
    #[serde(rename = "cookieHeader", skip_serializing_if = "Option::is_none")]
    pub(crate) cookie_header: Option<String>,
    #[serde(rename = "requestBody")]
    pub(crate) request_body: &'a Value,
    pub(crate) model: &'a str,
    #[serde(rename = "acceptAsyncJob")]
    pub(crate) accept_async_job: bool,
    #[serde(rename = "timeoutMs")]
    pub(crate) timeout_ms: u64,
    #[serde(
        rename = "browserExecutablePath",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) browser_executable_path: Option<String>,
    #[serde(rename = "userAgent", skip_serializing_if = "Option::is_none")]
    pub(crate) user_agent: Option<String>,
    #[serde(rename = "acceptLanguage", skip_serializing_if = "Option::is_none")]
    pub(crate) accept_language: Option<String>,
    #[serde(rename = "origin", skip_serializing_if = "Option::is_none")]
    pub(crate) origin: Option<String>,
    #[serde(rename = "referer", skip_serializing_if = "Option::is_none")]
    pub(crate) referer: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ProducerBrowserWorkerResult {
    pub(crate) ok: bool,
    pub(crate) status: Option<u16>,
    pub(crate) result: Option<Value>,
    pub(crate) error: Option<ProducerBrowserWorkerError>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ProducerBrowserWorkerError {
    pub(crate) code: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) status: Option<u16>,
    pub(crate) body: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct UdioBrowserWorkerInput<'a> {
    #[serde(rename = "baseUrl")]
    pub(crate) base_url: &'a str,
    #[serde(rename = "cookieHeader", skip_serializing_if = "Option::is_none")]
    pub(crate) cookie_header: Option<String>,
    #[serde(rename = "requestBody")]
    pub(crate) request_body: &'a Value,
    #[serde(rename = "targetAssetKind")]
    pub(crate) target_asset_kind: &'a str,
    #[serde(rename = "waitAudio")]
    pub(crate) wait_audio: bool,
    #[serde(rename = "waitTimeoutMs")]
    pub(crate) wait_timeout_ms: u64,
    #[serde(rename = "pollIntervalMs")]
    pub(crate) poll_interval_ms: u64,
    #[serde(rename = "timeoutMs")]
    pub(crate) timeout_ms: u64,
    #[serde(
        rename = "runtimeStateObjectKey",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) runtime_state_object_key: Option<String>,
    #[serde(
        rename = "browserExecutablePath",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) browser_executable_path: Option<String>,
    #[serde(rename = "userAgent", skip_serializing_if = "Option::is_none")]
    pub(crate) user_agent: Option<String>,
    #[serde(rename = "acceptLanguage", skip_serializing_if = "Option::is_none")]
    pub(crate) accept_language: Option<String>,
    #[serde(rename = "origin", skip_serializing_if = "Option::is_none")]
    pub(crate) origin: Option<String>,
    #[serde(rename = "referer", skip_serializing_if = "Option::is_none")]
    pub(crate) referer: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct UdioBrowserWorkerResult {
    pub(crate) ok: bool,
    pub(crate) status: Option<u16>,
    pub(crate) result: Option<UdioBrowserWorkerSuccess>,
    pub(crate) error: Option<UdioBrowserWorkerError>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct UdioBrowserWorkerSuccess {
    #[serde(rename = "trackIds", default)]
    pub(crate) track_ids: Vec<String>,
    #[serde(default)]
    pub(crate) songs: Vec<Value>,
    pub(crate) completed: bool,
    pub(crate) message: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct UdioBrowserWorkerError {
    pub(crate) code: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) status: Option<u16>,
    pub(crate) body: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct SunoBrowserWorkerInput<'a> {
    #[serde(rename = "baseUrl")]
    pub(crate) base_url: &'a str,
    #[serde(rename = "cookieHeader", skip_serializing_if = "Option::is_none")]
    pub(crate) cookie_header: Option<String>,
    #[serde(rename = "authToken", skip_serializing_if = "Option::is_none")]
    pub(crate) auth_token: Option<String>,
    #[serde(rename = "requestBody")]
    pub(crate) request_body: &'a Value,
    #[serde(rename = "targetAssetKind")]
    pub(crate) target_asset_kind: &'a str,
    #[serde(rename = "waitCompletion")]
    pub(crate) wait_completion: bool,
    #[serde(rename = "waitTimeoutMs")]
    pub(crate) wait_timeout_ms: u64,
    #[serde(rename = "pollIntervalMs")]
    pub(crate) poll_interval_ms: u64,
    #[serde(rename = "timeoutMs")]
    pub(crate) timeout_ms: u64,
    #[serde(
        rename = "browserExecutablePath",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) browser_executable_path: Option<String>,
    #[serde(rename = "userAgent", skip_serializing_if = "Option::is_none")]
    pub(crate) user_agent: Option<String>,
    #[serde(rename = "acceptLanguage", skip_serializing_if = "Option::is_none")]
    pub(crate) accept_language: Option<String>,
    #[serde(rename = "origin", skip_serializing_if = "Option::is_none")]
    pub(crate) origin: Option<String>,
    #[serde(rename = "referer", skip_serializing_if = "Option::is_none")]
    pub(crate) referer: Option<String>,
    #[serde(rename = "deviceId", skip_serializing_if = "Option::is_none")]
    pub(crate) device_id: Option<String>,
    #[serde(rename = "browserToken", skip_serializing_if = "Option::is_none")]
    pub(crate) browser_token: Option<String>,
    #[serde(rename = "referringPathname", skip_serializing_if = "Option::is_none")]
    pub(crate) referring_pathname: Option<String>,
    #[serde(rename = "referringOrigin", skip_serializing_if = "Option::is_none")]
    pub(crate) referring_origin: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct SunoBrowserWorkerResult {
    pub(crate) ok: bool,
    pub(crate) status: Option<u16>,
    pub(crate) result: Option<SunoBrowserWorkerSuccess>,
    pub(crate) error: Option<SunoBrowserWorkerError>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct SunoBrowserWorkerSuccess {
    #[serde(default)]
    pub(crate) clips: Vec<suno::SunoClip>,
    pub(crate) completed: bool,
    pub(crate) message: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct SunoBrowserWorkerError {
    pub(crate) code: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) status: Option<u16>,
    pub(crate) body: Option<String>,
}
