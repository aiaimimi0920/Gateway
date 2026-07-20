use std::collections::HashMap;

use rquest::header::HeaderMap;
use serde_json::Value;

use crate::protocol::canonical::EndpointKind;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::gemini::canvas_program_web_reverse as gemini_canvas_program_web_reverse_modular;

pub(crate) fn extract_gemini_canvas_batchexecute_xsrf_token(body: &str) -> Option<String> {
    let marker = "[\"xsrf\",\"";
    let start = body.find(marker)? + marker.len();
    let tail = &body[start..];
    let end = tail.find('"')?;
    let token = tail[..end].trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn extract_gemini_canvas_browser_pool_xsrf_token(body_text: &str) -> Option<String> {
    if let Some(token) = extract_gemini_canvas_batchexecute_xsrf_token(body_text) {
        return Some(token);
    }
    let result = serde_json::from_str::<
        gemini_canvas_program_web_reverse_modular::GeminiCanvasBrowserPoolResult,
    >(body_text)
    .ok()?;
    if let Some(body) = result.error.and_then(|entry| entry.body) {
        if let Some(token) = extract_gemini_canvas_batchexecute_xsrf_token(&body) {
            return Some(token);
        }
    }
    if let Some(result_body) = result.result {
        if let Some(body_text) = result_body.get("bodyText").and_then(Value::as_str) {
            if let Some(token) = extract_gemini_canvas_batchexecute_xsrf_token(body_text) {
                return Some(token);
            }
        }
    }
    None
}

pub(crate) fn header_map_string_from_form(form: &[(String, String)], key: &str) -> Option<String> {
    form.iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.to_string())
}

pub(crate) fn upsert_form_field(form: &mut Vec<(String, String)>, key: &str, value: &str) {
    if let Some((_, existing)) = form.iter_mut().find(|(name, _)| name == key) {
        *existing = value.to_string();
    } else {
        form.push((key.to_string(), value.to_string()));
    }
}

pub(crate) fn maybe_retry_gemini_canvas_form_xsrf_token(
    form: &mut Vec<(String, String)>,
    body_text: &str,
    last_retry_token: &mut Option<String>,
) -> Option<String> {
    let token = extract_gemini_canvas_browser_pool_xsrf_token(body_text)?;
    let current_at = header_map_string_from_form(form, "at");
    if last_retry_token.as_deref() == Some(token.as_str())
        || current_at.as_deref() == Some(token.as_str())
    {
        return None;
    }
    upsert_form_field(form, "at", &token);
    *last_retry_token = Some(token.clone());
    Some(token)
}

pub(crate) fn maybe_retry_gemini_canvas_stream_template_access_token(
    template: &mut crate::protocol::gemini_canvas::GeminiCanvasTextStreamGenerateTemplate,
    body_text: &str,
    retry_attempted: &mut bool,
) -> Option<String> {
    if *retry_attempted {
        return None;
    }

    let token = extract_gemini_canvas_batchexecute_xsrf_token(body_text)?;
    let current_at = header_map_string_from_form(&template.form, "at");
    if current_at.as_deref() == Some(token.as_str()) {
        return None;
    }

    *template = crate::protocol::gemini_canvas::refresh_stream_generate_template_access_token(
        template, &token,
    );
    *retry_attempted = true;
    Some(token)
}

pub(crate) fn serialize_form_urlencoded_pairs(form: &[(String, String)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in form {
        serializer.append_pair(name, value);
    }
    serializer.finish()
}

pub(crate) fn append_query_pairs_to_url(request_url: &str, query: &[(String, String)]) -> String {
    if query.is_empty() {
        return request_url.to_string();
    }
    if let Ok(mut parsed) = url::Url::parse(request_url) {
        {
            let mut pairs = parsed.query_pairs_mut();
            for (name, value) in query {
                pairs.append_pair(name, value);
            }
        }
        parsed.to_string()
    } else {
        let encoded_query = serialize_form_urlencoded_pairs(query);
        let separator = if request_url.contains('?') { '&' } else { '?' };
        format!("{request_url}{separator}{encoded_query}")
    }
}

pub(crate) fn gemini_canvas_browser_fetch_headers_from_header_map(
    headers: &HeaderMap,
) -> HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .collect()
}

pub(crate) fn should_attempt_gemini_canvas_browser_backed_image_edit_retry(
    payload: &ProviderAccountPayload,
    endpoint_kind: EndpointKind,
) -> bool {
    endpoint_kind == EndpointKind::ImagesEdits
        && payload.adapter == "gemini_web_reverse_modular_compatible"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::gemini_canvas;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
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
    fn browser_backed_image_edit_retry_is_limited_to_line2_edit_lane() {
        let line2_payload = make_payload(
            "gemini_web_reverse_modular_compatible",
            "https://gemini.google.com",
        );
        assert!(
            should_attempt_gemini_canvas_browser_backed_image_edit_retry(
                &line2_payload,
                EndpointKind::ImagesEdits
            )
        );
        assert!(
            !should_attempt_gemini_canvas_browser_backed_image_edit_retry(
                &line2_payload,
                EndpointKind::ImagesGenerations
            )
        );

        let line3_payload = make_payload(
            "gemini_canvas_program_web_reverse_compatible",
            "https://gemini.google.com",
        );
        assert!(
            !should_attempt_gemini_canvas_browser_backed_image_edit_retry(
                &line3_payload,
                EndpointKind::ImagesEdits
            )
        );
    }

    #[test]
    fn serialize_form_urlencoded_pairs_and_query_append_preserve_stream_payload_shape() {
        let form = vec![
            ("f.req".to_string(), "[[\"hello world\"]]".to_string()),
            ("at".to_string(), "token:123".to_string()),
        ];
        let encoded = serialize_form_urlencoded_pairs(&form);
        assert!(encoded.contains("f.req=%5B%5B%22hello+world%22%5D%5D"));
        assert!(encoded.contains("at=token%3A123"));

        let final_url = append_query_pairs_to_url(
            "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate",
            &[
                ("hl".to_string(), "zh-CN".to_string()),
                ("_reqid".to_string(), "12345".to_string()),
            ],
        );
        assert!(final_url.contains("hl=zh-CN"));
        assert!(final_url.contains("_reqid=12345"));
    }

    #[test]
    fn browser_fetch_xsrf_retry_updates_form_token_from_pool_error_body_once() {
        let mut form = vec![
            ("f.req".to_string(), "payload".to_string()),
            ("at".to_string(), "old-token".to_string()),
        ];
        let mut xsrf_retry_token = None;
        let pool_error = r#"{"ok":false,"status":400,"error":{"status":400,"body":")]}'

19
[["er",null,null,null,null,400,null,null,null,3,[{"48448350":["xsrf","AOOh0PGSwKMJsw1I9iNkxrnWE4jJ:1778770951003",["106807866802275293212"]]}]],["di",19]]"}}"#;

        let token =
            maybe_retry_gemini_canvas_form_xsrf_token(&mut form, pool_error, &mut xsrf_retry_token)
                .expect("xsrf retry token extracted");
        assert_eq!(token, "AOOh0PGSwKMJsw1I9iNkxrnWE4jJ:1778770951003");
        assert_eq!(
            header_map_string_from_form(&form, "at").as_deref(),
            Some("AOOh0PGSwKMJsw1I9iNkxrnWE4jJ:1778770951003")
        );
        assert_eq!(
            xsrf_retry_token.as_deref(),
            Some("AOOh0PGSwKMJsw1I9iNkxrnWE4jJ:1778770951003")
        );

        assert!(
            maybe_retry_gemini_canvas_form_xsrf_token(
                &mut form,
                pool_error,
                &mut xsrf_retry_token,
            )
            .is_none(),
            "same token should not trigger an infinite retry loop"
        );
    }

    #[test]
    fn stream_template_xsrf_retry_refreshes_access_token_once() {
        let mut template = gemini_canvas::GeminiCanvasTextStreamGenerateTemplate {
            url: "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate".to_string(),
            query: vec![("hl".to_string(), "zh-CN".to_string())],
            form: vec![
                ("f.req".to_string(), "payload".to_string()),
                ("at".to_string(), "old-token".to_string()),
            ],
            raw_post_data: "f.req=payload&at=old-token".to_string(),
            headers: HashMap::new(),
        };
        let mut retry_attempted = false;
        let body = r#"[["xsrf","new-token",["123"]]]"#;

        let token = maybe_retry_gemini_canvas_stream_template_access_token(
            &mut template,
            body,
            &mut retry_attempted,
        )
        .expect("xsrf retry token");

        assert_eq!(token, "new-token");
        assert!(retry_attempted);
        assert_eq!(
            header_map_string_from_form(&template.form, "at").as_deref(),
            Some("new-token")
        );
        assert!(template.raw_post_data.contains("at=new-token"));

        assert!(
            maybe_retry_gemini_canvas_stream_template_access_token(
                &mut template,
                body,
                &mut retry_attempted,
            )
            .is_none(),
            "second retry should not re-fire once attempt flag is set"
        );
    }

    #[test]
    fn stream_template_xsrf_retry_skips_when_template_already_has_token() {
        let mut template = gemini_canvas::GeminiCanvasTextStreamGenerateTemplate {
            url: "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate".to_string(),
            query: vec![],
            form: vec![
                ("f.req".to_string(), "payload".to_string()),
                ("at".to_string(), "same-token".to_string()),
            ],
            raw_post_data: "f.req=payload&at=same-token".to_string(),
            headers: HashMap::new(),
        };
        let mut retry_attempted = false;
        let body = r#"[["xsrf","same-token",["123"]]]"#;

        assert!(
            maybe_retry_gemini_canvas_stream_template_access_token(
                &mut template,
                body,
                &mut retry_attempted,
            )
            .is_none(),
            "same token should not trigger template refresh"
        );
        assert!(!retry_attempted);
        assert_eq!(
            header_map_string_from_form(&template.form, "at").as_deref(),
            Some("same-token")
        );
    }
}
