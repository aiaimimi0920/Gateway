//! Local and remote browser executor service wire contracts.

use serde_json::Value;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserExecutorInvocationRequest<'a> {
    pub(crate) provider: &'a str,
    pub(crate) provider_account_id: &'a str,
    pub(crate) endpoint_kind: &'a str,
    pub(crate) execution_mode: &'a str,
    pub(crate) input: Value,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserExecutorInvocationResponse {
    pub(crate) ok: bool,
    pub(crate) status: Option<u16>,
    pub(crate) result: Option<Value>,
    pub(crate) error: Option<BrowserExecutorInvocationError>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorInvocationError {
    pub code: Option<String>,
    pub message: Option<String>,
    pub status: Option<u16>,
    pub body: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorServiceInvocationRequest {
    pub provider: String,
    pub provider_account_id: String,
    pub endpoint_kind: String,
    pub execution_mode: Option<String>,
    pub input: Value,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorServiceHealth {
    pub ok: bool,
    pub enabled: bool,
    pub mode: String,
    pub remote_base_url: Option<String>,
    pub lumalabs_script_path: String,
    pub producer_script_path: String,
    pub suno_script_path: String,
    pub udio_script_path: String,
    pub gemini_canvas_pool_script_path: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorServiceInvocationResponse {
    pub ok: bool,
    pub provider: String,
    pub status: Option<u16>,
    pub result: Option<Value>,
    pub error: Option<BrowserExecutorInvocationError>,
    pub lease: Option<Value>,
    pub browser_execution_status: String,
}
