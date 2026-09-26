use super::*;

#[test]
fn direct_http_google_api_key_prefers_payload_extra_body() {
    let mut extra_body = HashMap::new();
    extra_body.insert(
        "googleApiKey".to_string(),
        Value::String("AIzaPayloadKey123".to_string()),
    );
    extra_body.insert(
        "apiKeys".to_string(),
        Value::Array(vec![Value::String("AIzaArrayKey456".to_string())]),
    );
    let payload = ProviderAccountPayload {
        adapter: "gemini_canvas_compatible".to_string(),
        base_url: "https://gemini.google.com".to_string(),
        api_key: "AIzaPayloadKeyZero".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: Some(
            "credential-runtime/gemini-canvas/demo/storage-state.json".to_string(),
        ),
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
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
        extra_body: Some(extra_body),
        session_auth: None,
        keepalive: None,
    };

    let key = direct_http_google_api_key(
        &payload,
        &json!({
            "apiKeys": ["AIzaStorageKey789"]
        }),
    );
    let keys = direct_http_google_api_keys(
        &payload,
        &json!({
            "apiKeys": ["AIzaStorageKey789"]
        }),
    );

    assert_eq!(key.as_deref(), Some("AIzaPayloadKey123"));
    assert_eq!(
        keys,
        vec![
            "AIzaPayloadKey123".to_string(),
            "AIzaArrayKey456".to_string(),
            "AIzaStorageKey789".to_string()
        ]
    );
}

#[test]
fn direct_http_google_api_key_reads_storage_state_metadata() {
    let payload = ProviderAccountPayload {
        adapter: "gemini_canvas_compatible".to_string(),
        base_url: "https://gemini.google.com".to_string(),
        api_key: "AIzaPayloadKeyZero".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: Some(
            "credential-runtime/gemini-canvas/demo/storage-state.json".to_string(),
        ),
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
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
        extra_body: Some(HashMap::new()),
        session_auth: None,
        keepalive: None,
    };

    let top_level = direct_http_google_api_key(
        &payload,
        &json!({
            "apiKeys": ["AIzaTopLevel123"]
        }),
    );
    let firebase = direct_http_google_api_key(
        &payload,
        &json!({
            "firebaseConfig": {
                "apiKey": "AIzaFirebase456"
            }
        }),
    );

    assert_eq!(top_level.as_deref(), Some("AIzaTopLevel123"));
    assert_eq!(firebase.as_deref(), Some("AIzaFirebase456"));
}

#[test]
fn direct_http_google_api_keys_dedupes_all_known_sources() {
    let mut extra_body = HashMap::new();
    extra_body.insert(
        "apiKeys".to_string(),
        Value::Array(vec![
            Value::String("AIzaKeyOne".to_string()),
            Value::String("AIzaKeyTwo".to_string()),
            Value::String("AIzaKeyOne".to_string()),
        ]),
    );
    let payload = ProviderAccountPayload {
        adapter: "gemini_canvas_compatible".to_string(),
        base_url: "https://gemini.google.com".to_string(),
        api_key: "AIzaPayloadKeyZero".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: Some(
            "credential-runtime/gemini-canvas/demo/storage-state.json".to_string(),
        ),
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
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
        extra_body: Some(extra_body),
        session_auth: None,
        keepalive: None,
    };

    let keys = direct_http_google_api_keys(
        &payload,
        &json!({
            "googleApiKey": "AIzaKeyTwo",
            "apiKeys": ["AIzaKeyThree"],
            "firebaseConfig": {
                "apiKey": "AIzaKeyFour",
                "apiKeys": ["AIzaKeyThree", "AIzaKeyFive"]
            }
        }),
    );

    assert_eq!(
        keys,
        vec![
            "AIzaPayloadKeyZero".to_string(),
            "AIzaKeyOne".to_string(),
            "AIzaKeyTwo".to_string(),
            "AIzaKeyThree".to_string(),
            "AIzaKeyFour".to_string(),
            "AIzaKeyFive".to_string()
        ]
    );
}

#[test]
fn extract_google_api_keys_from_page_blob_collects_unique_matches() {
    let blob = r#"
            <html><body>
            <script>
              window.firebaseConfig = {"apiKey":"AIzaBlobKey123456789012345"};
              const more = ["AIzaBlobKeySecond123456789012345", "AIzaBlobKey123456789012345"];
            </script>
            </body></html>
        "#;

    let keys = extract_google_api_keys_from_page_blob(blob);

    assert_eq!(
        keys,
        vec![
            "AIzaBlobKey123456789012345".to_string(),
            "AIzaBlobKeySecond123456789012345".to_string()
        ]
    );
}
