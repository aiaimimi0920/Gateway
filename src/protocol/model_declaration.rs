//! Raw provider metadata, deliberately separate from fallback and routing model names.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDeclaration {
    pub value: String,
    pub source: ModelDeclarationSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelDeclarationSource {
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "message.model")]
    MessageModel,
    #[serde(rename = "response.model")]
    ResponseModel,
}

impl ModelDeclaration {
    pub fn from_body(body: &Value) -> Option<Self> {
        Self::from_value(body.get("model")?, ModelDeclarationSource::Model)
    }

    pub(crate) fn from_value(value: &Value, source: ModelDeclarationSource) -> Option<Self> {
        Some(Self {
            value: bounded_model_label(value.as_str()?)?,
            source,
        })
    }
}

/// Reject rather than truncate: a shortened identifier would manufacture evidence.
/// No body text, control characters, bearer values or conventional secret prefixes.
pub(crate) fn bounded_model_label(value: &str) -> Option<String> {
    if value.is_empty()
        || value.len() > 256
        || value.starts_with("sk-")
        || value.starts_with("eyJ")
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._:-+@".contains(&b))
    {
        return None;
    }
    Some(value.to_string())
}
