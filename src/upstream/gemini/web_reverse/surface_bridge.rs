use std::collections::HashMap;

use serde_json::Value;

use crate::protocol::gemini::web_reverse as surface;

pub fn bootstrap_from_payload_cache(
    extra_body: Option<&HashMap<String, Value>>,
) -> Option<surface::GeminiWebBootstrap> {
    surface::bootstrap_from_payload_cache(extra_body)
}
