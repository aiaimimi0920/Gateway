//! Gemini HTTP replay worker request and response wire contracts.

use std::collections::HashMap;

#[derive(Debug, serde::Serialize)]
pub(crate) struct GeminiCanvasHttpReplayWorkerInput<'a> {
    pub(crate) url: &'a str,
    pub(crate) query: &'a [(String, String)],
    pub(crate) headers: &'a HashMap<String, String>,
    #[serde(rename = "rawPostData")]
    pub(crate) raw_post_data: &'a str,
    #[serde(rename = "cookieHeader")]
    pub(crate) cookie_header: &'a str,
    #[serde(rename = "operation", skip_serializing_if = "Option::is_none")]
    pub(crate) operation: Option<&'a str>,
    #[serde(rename = "timeoutMs")]
    pub(crate) timeout_ms: u64,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct GeminiCanvasHttpReplayWorkerResult {
    pub(crate) ok: bool,
    pub(crate) status: Option<u16>,
    #[serde(rename = "contentType")]
    pub(crate) content_type: Option<String>,
    #[serde(rename = "bodyText")]
    pub(crate) body_text: Option<String>,
    pub(crate) error: Option<GeminiCanvasHttpReplayWorkerError>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct GeminiCanvasHttpReplayWorkerError {
    pub(crate) code: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) status: Option<u16>,
    #[serde(rename = "bodyText")]
    pub(crate) body_text: Option<String>,
}

#[derive(Debug)]
pub(crate) struct GeminiCanvasHttpReplayWorkerSuccess {
    pub(crate) status: u16,
    pub(crate) content_type: Option<String>,
    pub(crate) body_text: String,
}
