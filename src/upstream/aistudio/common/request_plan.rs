use std::collections::HashMap;

use serde_json::Value;

use crate::protocol::aistudio_web;

#[derive(Debug, Clone)]
pub struct AIStudioJsonRequestPlan {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Value,
}

impl AIStudioJsonRequestPlan {
    pub fn to_browser_request_spec(&self) -> aistudio_web::AIStudioBrowserRequestSpec {
        aistudio_web::build_browser_request_spec(
            self.method.as_str(),
            self.url.clone(),
            self.headers.clone(),
            Some(self.body.to_string()),
        )
    }
}

pub fn build_generate_content_request_plan(
    base_url: &str,
    model: &str,
    headers: HashMap<String, String>,
    body: Value,
) -> AIStudioJsonRequestPlan {
    AIStudioJsonRequestPlan {
        method: "POST".to_string(),
        url: aistudio_web::build_generate_content_url(base_url, model),
        headers,
        body,
    }
}

pub fn build_embeddings_request_plan(
    request: aistudio_web::AIStudioEmbeddingsRequest,
    headers: HashMap<String, String>,
) -> AIStudioJsonRequestPlan {
    AIStudioJsonRequestPlan {
        method: "POST".to_string(),
        url: request.url,
        headers,
        body: request.body,
    }
}
