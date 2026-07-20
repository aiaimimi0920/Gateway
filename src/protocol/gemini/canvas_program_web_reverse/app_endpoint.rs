#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeminiCanvasProgramInvokeTargetCandidate {
    pub url: Option<String>,
    pub source: Option<String>,
    pub mime_type: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeminiCanvasProgramInvokeContract {
    pub operation: Option<String>,
    pub transport_kind: Option<String>,
    pub target: Option<String>,
    pub target_source: Option<String>,
    pub target_mime_type: Option<String>,
    pub target_candidates: Vec<GeminiCanvasProgramInvokeTargetCandidate>,
    pub ws_url: Option<String>,
    pub api_style: Option<String>,
    pub request_path: Option<String>,
    pub request_envelope_kind: Option<String>,
    pub request_url: Option<String>,
    pub request_body: Option<String>,
    pub request_rpc_id: Option<String>,
    pub response_rpc_id: Option<String>,
    pub source_path: Option<String>,
    pub model_hint: Option<String>,
    pub cookie_header: Option<String>,
    pub action_name: Option<String>,
    pub action_input: Option<String>,
    pub prompt: Option<String>,
    pub duration_seconds: Option<f64>,
    pub aspect_ratio: Option<String>,
    pub ui_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeminiCanvasProgramAppEndpointContract {
    pub canvas_program_url: Option<String>,
    pub page_url: Option<String>,
    pub app_path: Option<String>,
    pub conversation_id: Option<String>,
    pub response_id: Option<String>,
    pub invoke_base_url: Option<String>,
    pub music_ws_url: Option<String>,
    pub video_invoke_path: Option<String>,
    pub canvas_program_action: Option<String>,
    pub canvas_program_action_input: Option<String>,
    pub canvas_program_invoke_contract: Option<GeminiCanvasProgramInvokeContract>,
}

impl GeminiCanvasProgramAppEndpointContract {
    pub fn has_concrete_handle(&self) -> bool {
        self.canvas_program_url
            .as_deref()
            .is_some_and(is_concrete_gemini_canvas_program_url)
            || self
                .app_path
                .as_deref()
                .is_some_and(is_concrete_gemini_canvas_app_path)
            || self
                .conversation_id
                .as_deref()
                .is_some_and(is_concrete_gemini_canvas_conversation_id)
    }
}

fn is_concrete_gemini_canvas_program_url(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    if let Ok(parsed) = url::Url::parse(trimmed) {
        return matches!(
            parsed.domain(),
            Some("gemini.google.com") | Some("www.gemini.google.com")
        ) && parsed.path().starts_with("/app/")
            && parsed
                .path_segments()
                .and_then(|segments| segments.last())
                .map(|segment| !segment.is_empty())
                .unwrap_or(false);
    }
    trimmed.starts_with("/app/") && trimmed.len() > "/app/".len()
}

fn is_concrete_gemini_canvas_app_path(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("/app/") && trimmed.len() > "/app/".len()
}

fn is_concrete_gemini_canvas_conversation_id(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("c_") && trimmed.len() > 2
}
