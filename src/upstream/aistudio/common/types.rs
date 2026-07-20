use crate::protocol::aistudio_web;

#[derive(Debug, serde::Serialize)]
pub struct AIStudioBrowserWorkerInput<'a> {
    #[serde(rename = "runtimeStateObjectKey")]
    pub runtime_state_object_key: &'a str,
    #[serde(rename = "appUrl")]
    pub app_url: &'a str,
    #[serde(rename = "requestSpec")]
    pub request_spec: &'a aistudio_web::AIStudioBrowserRequestSpec,
    #[serde(rename = "timeoutMs")]
    pub timeout_ms: u64,
    #[serde(rename = "resultFilePath", skip_serializing_if = "Option::is_none")]
    pub result_file_path: Option<String>,
    #[serde(
        rename = "browserExecutablePath",
        skip_serializing_if = "Option::is_none"
    )]
    pub browser_executable_path: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct AIStudioBrowserWorkerResult {
    pub ok: bool,
    pub status: Option<u16>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    #[serde(rename = "bodyText")]
    pub body_text: Option<String>,
    #[serde(rename = "bodyFilePath")]
    pub body_file_path: Option<String>,
    #[serde(rename = "bodyBase64")]
    pub body_base64: Option<String>,
    pub error: Option<AIStudioBrowserWorkerError>,
}

#[derive(Debug, serde::Deserialize)]
pub struct AIStudioBrowserWorkerError {
    pub code: Option<String>,
    pub message: Option<String>,
    pub status: Option<u16>,
    pub body: Option<String>,
}
