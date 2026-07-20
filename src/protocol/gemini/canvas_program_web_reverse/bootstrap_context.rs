#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasProgramBootstrapContext {
    pub runtime_state_object_key: String,
    pub share_id: String,
    pub api_base_url: String,
    pub relay_ws_endpoint: Option<String>,
    pub client_label: Option<String>,
    pub canvas_program_hint: Option<String>,
}
