use std::collections::HashMap;

use serde_json::Value;

use crate::routing::candidate::ProviderAccountPayload;

pub(crate) fn read_json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
}

pub(crate) fn merge_extra_body_patch_into_payload(
    payload: &ProviderAccountPayload,
    extra_body_patch: &HashMap<String, Value>,
) -> ProviderAccountPayload {
    let mut cloned = payload.clone();
    let extra_body = cloned.extra_body.get_or_insert_with(HashMap::new);
    for (key, value) in extra_body_patch {
        extra_body.insert(key.clone(), value.clone());
    }
    cloned
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    #[test]
    fn read_json_string_trims_and_filters_empty_values() {
        let value = json!({
            "baseUrl": "  https://example.com  ",
            "empty": "   ",
            "number": 123
        });
        assert_eq!(
            read_json_string(&value, "baseUrl").as_deref(),
            Some("https://example.com")
        );
        assert_eq!(read_json_string(&value, "empty"), None);
        assert_eq!(read_json_string(&value, "number"), None);
    }

    #[test]
    fn merge_extra_body_patch_into_payload_overlays_patch_without_losing_existing_fields() {
        let mut payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        payload.extra_body = Some(HashMap::from([
            ("existing".to_string(), json!("keep")),
            ("overwrite".to_string(), json!("old")),
        ]));
        let merged = merge_extra_body_patch_into_payload(
            &payload,
            &HashMap::from([
                ("overwrite".to_string(), json!("new")),
                ("added".to_string(), json!(true)),
            ]),
        );
        let extra_body = merged
            .extra_body
            .expect("merged payload should carry extra body");
        assert_eq!(extra_body.get("existing"), Some(&json!("keep")));
        assert_eq!(extra_body.get("overwrite"), Some(&json!("new")));
        assert_eq!(extra_body.get("added"), Some(&json!(true)));
    }
}
