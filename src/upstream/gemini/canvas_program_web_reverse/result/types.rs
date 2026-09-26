use serde_json::Value;

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserPoolResult {
    pub ok: bool,
    pub status: Option<u16>,
    pub result: Option<Value>,
    pub error: Option<GeminiCanvasBrowserPoolError>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserPoolError {
    pub code: Option<String>,
    pub message: Option<String>,
    pub status: Option<u16>,
    pub body: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserInvocationResult {
    pub operation: String,
    #[serde(rename = "bootstrapOperation")]
    pub bootstrap_operation: Option<String>,
    #[serde(rename = "shareUrl")]
    pub share_url: Option<String>,
    #[serde(rename = "shareId")]
    pub share_id: Option<String>,
    #[serde(rename = "shareFollowKind")]
    pub share_follow_kind: Option<String>,
    #[serde(rename = "beforeUrl")]
    pub before_url: Option<String>,
    #[serde(rename = "finalUrl")]
    pub final_url: Option<String>,
    #[serde(rename = "pageUrl")]
    pub page_url: Option<String>,
    #[serde(rename = "canvasProgramUrl")]
    pub canvas_program_url: Option<String>,
    #[serde(rename = "appPath")]
    pub app_path: Option<String>,
    #[serde(rename = "conversationId")]
    pub conversation_id: Option<String>,
    #[serde(rename = "responseId")]
    pub response_id: Option<String>,
    #[serde(rename = "invokeBaseUrl")]
    pub invoke_base_url: Option<String>,
    #[serde(rename = "musicWsUrl")]
    pub music_ws_url: Option<String>,
    #[serde(rename = "videoInvokePath")]
    pub video_invoke_path: Option<String>,
    #[serde(rename = "canvasProgramAction")]
    pub canvas_program_action: Option<String>,
    #[serde(rename = "canvasProgramActionInput")]
    pub canvas_program_action_input: Option<String>,
    #[serde(rename = "canvasProgramInvokeContract")]
    pub canvas_program_invoke_contract: Option<Value>,
    #[serde(rename = "lastSeenConversationId")]
    pub last_seen_conversation_id: Option<String>,
    #[serde(rename = "lastSeenResponseId")]
    pub last_seen_response_id: Option<String>,
    #[serde(rename = "candidatePairs", default)]
    pub candidate_pairs: Vec<Value>,
    #[serde(rename = "stableProgramPair")]
    pub stable_program_pair: Option<Value>,
    #[serde(rename = "latestResponsePair")]
    pub latest_response_pair: Option<Value>,
    #[serde(rename = "aggregateHints")]
    pub aggregate_hints: Option<Value>,
    #[serde(rename = "capturedAt")]
    pub captured_at: Option<String>,
    #[serde(rename = "lastValidatedAt")]
    pub last_validated_at: Option<String>,
    #[serde(rename = "newChatClicked")]
    pub new_chat_clicked: Option<bool>,
    #[serde(rename = "modeSelected")]
    pub mode_selected: Option<bool>,
    #[serde(rename = "bodyText")]
    pub body_text: Option<String>,
    #[serde(rename = "bodyBase64")]
    pub body_base64: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub text: Option<String>,
    #[serde(default)]
    pub media: Vec<GeminiCanvasBrowserMediaAsset>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GeminiCanvasBrowserMediaAsset {
    pub kind: String,
    pub url: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "bodyBase64")]
    pub body_base64: Option<String>,
    pub alt: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(rename = "durationSeconds")]
    pub duration_seconds: Option<f64>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserFetchInvocationResult {
    pub operation: String,
    pub status: u16,
    #[serde(rename = "finalUrl")]
    pub final_url: Option<String>,
    #[serde(rename = "pageUrl")]
    pub page_url: Option<String>,
    #[serde(rename = "canvasProgramUrl")]
    pub canvas_program_url: Option<String>,
    #[serde(rename = "appPath")]
    pub app_path: Option<String>,
    #[serde(rename = "conversationId")]
    pub conversation_id: Option<String>,
    #[serde(rename = "responseId")]
    pub response_id: Option<String>,
    #[serde(rename = "invokeBaseUrl")]
    pub invoke_base_url: Option<String>,
    #[serde(rename = "musicWsUrl")]
    pub music_ws_url: Option<String>,
    #[serde(rename = "videoInvokePath")]
    pub video_invoke_path: Option<String>,
    #[serde(rename = "lastSeenConversationId")]
    pub last_seen_conversation_id: Option<String>,
    #[serde(rename = "lastSeenResponseId")]
    pub last_seen_response_id: Option<String>,
    #[serde(rename = "candidatePairs", default)]
    pub candidate_pairs: Vec<Value>,
    #[serde(rename = "capturedAt")]
    pub captured_at: Option<String>,
    #[serde(rename = "lastValidatedAt")]
    pub last_validated_at: Option<String>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(rename = "bodyText")]
    pub body_text: Option<String>,
    #[serde(rename = "bodyBase64")]
    pub body_base64: Option<String>,
}
