use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AIStudioBrowserRequestSpec {
    pub method: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

pub fn build_browser_request_spec(
    method: impl Into<String>,
    url: impl Into<String>,
    headers: HashMap<String, String>,
    body: Option<String>,
) -> AIStudioBrowserRequestSpec {
    AIStudioBrowserRequestSpec {
        method: method.into(),
        url: url.into(),
        headers,
        body,
    }
}
