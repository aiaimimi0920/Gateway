use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasImageEditUpload {
    pub mime_type: String,
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub source_mime_type: String,
    pub source_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasUploadedFileRef {
    pub resource_path: String,
    pub mime_type: String,
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasStreamGenerateSeed {
    pub opaque_state: Option<String>,
    pub request_hex: Option<String>,
    pub request_uuid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasRuntime {
    pub runtime_state_object_key: String,
    pub share_id: String,
    pub api_base_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasPureHttpSession {
    pub cookie_header: String,
    pub sapisid: String,
    pub auth_user: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasTextStreamGenerateTemplate {
    pub url: String,
    pub query: Vec<(String, String)>,
    pub form: Vec<(String, String)>,
    pub raw_post_data: String,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasStreamGenerateLocator {
    pub response_id: String,
    pub conversation_id: String,
    pub app_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasConversationListEntry {
    pub conversation_id: String,
    pub title: String,
    pub response_id: Option<String>,
    pub updated_at_secs: i64,
    pub updated_at_nanos: i64,
}

impl GeminiCanvasConversationListEntry {
    pub fn app_path(&self) -> Option<String> {
        let app_conversation = self
            .conversation_id
            .strip_prefix("c_")
            .unwrap_or(self.conversation_id.as_str())
            .trim();
        if app_conversation.is_empty() {
            None
        } else {
            Some(format!("/app/{app_conversation}"))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiCanvasMediaOperation {
    Image,
    Music,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiCanvasImageModel {
    Gemini25FlashImagePreview,
    Gemini25FlashImage,
    Gemini31FlashImagePreview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasImage {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasAudio {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeminiCanvasMediaAsset {
    pub kind: String,
    pub url: String,
    pub mime_type: String,
    pub download_token: Option<String>,
    pub body_base64: Option<String>,
    pub alt: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_seconds: Option<f64>,
}
