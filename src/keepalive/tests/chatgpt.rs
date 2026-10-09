use super::super::*;
use axum::{extract::State, http::HeaderMap as AxumHeaderMap, routing::post, Json, Router};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

fn chatgpt_web_payload_for_oauth_refresh(token_endpoint: &str) -> ProviderAccountPayload {
    ProviderAccountPayload {
        discovered_protocols: Vec::new(),
        adapter: "chatgpt_web_reverse_compatible".to_string(),
        base_url: "https://chatgpt.com".to_string(),
        api_key: "expired-access-token".to_string(),
        credential_id: Some("chatgpt-web-cred-1".to_string()),
        expires_at: Some("2000-01-01T00:00:00Z".to_string()),
        runtime_state_object_key: None,
        account_name: Some("chatgpt-web".to_string()),
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some("gpt-5.4".to_string()),
        headers: HashMap::new(),
        auth_mode: Some("bearer".to_string()),
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
        extra_body: Some(HashMap::from([
            ("refreshToken".to_string(), json!("old-refresh-token")),
            ("oauthTokenEndpoint".to_string(), json!(token_endpoint)),
            ("oauthClientId".to_string(), json!("app-test-client")),
        ])),
        session_auth: Some(SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: Some("2000-01-01T00:00:00Z".to_string()),
        }),
        keepalive: None,
    }
}

#[test]
fn chatgpt_web_request_time_browser_global_policy_overrides_provider_allowed() {
    let payload = chatgpt_web_payload_for_oauth_refresh("http://127.0.0.1/oauth/token");

    assert!(
        !chatgpt_web_request_time_browser_fallback_allowed_for_policy(
            &payload,
            RequestTimeBrowserPolicy::Disabled
        )
    );
    assert!(
        !chatgpt_web_request_time_browser_fallback_allowed_for_policy(
            &payload,
            RequestTimeBrowserPolicy::RemoteOnly
        )
    );
    assert!(
        chatgpt_web_request_time_browser_fallback_allowed_for_policy(
            &payload,
            RequestTimeBrowserPolicy::LocalAllowed
        )
    );
}

#[test]
fn chatgpt_web_request_time_browser_local_policy_keeps_provider_disable() {
    let mut payload = chatgpt_web_payload_for_oauth_refresh("http://127.0.0.1/oauth/token");
    payload
        .extra_body
        .get_or_insert_with(HashMap::new)
        .insert("requestTimeBrowserAllowed".to_string(), json!(false));

    assert!(
        !chatgpt_web_request_time_browser_fallback_allowed_for_policy(
            &payload,
            RequestTimeBrowserPolicy::LocalAllowed
        )
    );
}

#[tokio::test]
async fn chatgpt_web_oauth_refresh_posts_form_and_returns_rotated_tokens() {
    #[derive(Clone, Default)]
    struct OAuthState {
        seen: Arc<Mutex<Vec<(Option<String>, String)>>>,
    }

    async fn record_oauth_refresh(
        State(state): State<OAuthState>,
        headers: AxumHeaderMap,
        body: String,
    ) -> Json<Value> {
        state.seen.lock().unwrap().push((
            headers
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
            body,
        ));
        Json(json!({
            "access_token": "refreshed-access-token",
            "refresh_token": "rotated-refresh-token",
            "id_token": "id-token-123",
            "expires_in": 3600
        }))
    }

    let state = OAuthState::default();
    let app = Router::new()
        .route("/oauth/token", post(record_oauth_refresh))
        .with_state(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("axum serve");
    });

    let payload = chatgpt_web_payload_for_oauth_refresh(&format!("http://{addr}/oauth/token"));
    let refreshed = execute_chatgpt_web_oauth_refresh(&Client::new(), &payload)
        .await
        .expect("oauth refresh");

    assert_eq!(refreshed.api_key, "refreshed-access-token");
    assert_eq!(
        refreshed.refresh_token.as_deref(),
        Some("rotated-refresh-token")
    );
    assert_eq!(refreshed.id_token.as_deref(), Some("id-token-123"));
    assert!(refreshed.expires_at.is_some());

    let seen = state.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    assert!(seen[0]
        .0
        .as_deref()
        .is_some_and(|value| value.starts_with("application/x-www-form-urlencoded")));
    assert!(seen[0].1.contains("grant_type=refresh_token"));
    assert!(seen[0].1.contains("refresh_token=old-refresh-token"));
    assert!(seen[0].1.contains("client_id=app-test-client"));

    server.abort();
}

#[test]
fn chatgpt_web_runtime_header_merge_drops_internal_account_group_selectors() {
    let headers = HashMap::from([
        (
            "X-Account-Group-Id".to_string(),
            "legacy-premium".to_string(),
        ),
        ("Accept".to_string(), "application/json".to_string()),
    ]);

    let merged = merge_chatgpt_web_runtime_headers(&headers, Some("a=b"), None);

    assert!(!merged
        .keys()
        .any(|name| name.eq_ignore_ascii_case("x-account-group-id")));
    assert_eq!(
        merged.get("Accept").map(String::as_str),
        Some("application/json")
    );
    assert_eq!(merged.get("Cookie").map(String::as_str), Some("a=b"));
}
