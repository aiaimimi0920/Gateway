#[derive(Debug, Clone)]
pub struct GeminiWebBootstrap {
    pub access_token: Option<String>,
    pub build_label: Option<String>,
    pub session_id: Option<String>,
    pub language: String,
    pub push_id: Option<String>,
    pub client_pctx: Option<String>,
    pub app_page_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GeminiWebRequest {
    pub query: Vec<(String, String)>,
    pub form: Vec<(String, String)>,
}
