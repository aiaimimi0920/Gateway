use super::super::*;
use axum::{extract::State, http::HeaderMap as AxumHeaderMap, routing::post, Json, Router};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[test]
fn extract_cookie_value_reads_named_entries() {
    let cookie_header = "__client=refresh-123; __session=session-456; other=1";
    assert_eq!(
        extract_cookie_value(cookie_header, "__session").as_deref(),
        Some("session-456")
    );
    assert_eq!(
        extract_cookie_value(cookie_header, "__client").as_deref(),
        Some("refresh-123")
    );
    assert_eq!(extract_cookie_value(cookie_header, "missing"), None);
}

#[test]
fn suno_cookie_header_from_storage_state_reads_suno_cookies_only() {
    let state = serde_json::json!({
        "cookies": [
            {"name": "__session", "value": "session-456", "domain": ".suno.com"},
            {"name": "device", "value": "device-1", "domain": "studio-api-prod.suno.com"},
            {"name": "unrelated", "value": "ignore-me", "domain": ".example.com"}
        ]
    });

    let cookie_header = suno_cookie_header_from_storage_state(&state).expect("Suno cookies");
    assert!(cookie_header.contains("__session=session-456"));
    assert!(cookie_header.contains("device=device-1"));
    assert!(!cookie_header.contains("unrelated"));
}

#[test]
fn suno_cookie_header_ignores_expired_and_duplicate_session_cookies() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_secs_f64();
    let future = now + 3_600.0;
    let past = now - 3_600.0;
    let state = serde_json::json!({
        "cookies": [
            {
                "name": "__session",
                "value": "expired-session",
                "domain": ".suno.com",
                "path": "/",
                "expires": past
            },
            {
                "name": "__session",
                "value": "current-session",
                "domain": "suno.com",
                "path": "/",
                "expires": future
            }
        ]
    });

    let cookie_header = suno_cookie_header_from_storage_state(&state).expect("Suno cookies");
    assert!(cookie_header.contains("__session=current-session"));
    assert!(!cookie_header.contains("expired-session"));
    assert_eq!(cookie_header.matches("__session=").count(), 1);
}

#[test]
fn read_suno_cookie_header_prefers_explicit_cookie_header() {
    let mut headers = HashMap::new();
    headers.insert("Cookie".to_string(), "__session=session-789".to_string());
    let request = GatewayKeepaliveEnsureRequest {
        project_id: None,
        session_key: None,
        previous_response_id: None,
        credential_id: None,
        account_name: None,
        provider_account_id: "provider-1".to_string(),
        adapter: "suno_compatible".to_string(),
        base_url: "https://studio-api-prod.suno.com".to_string(),
        model: "chirp-v3-5".to_string(),
        api_key: Some("__client=refresh-123".to_string()),
        headers,
        extra_body: None,
        session_auth: None,
        expires_at: None,
        runtime_state_object_key: None,
    };
    assert_eq!(
        read_suno_cookie_header(&request).as_deref(),
        Some("__session=session-789")
    );
}

#[tokio::test]
async fn ensure_suno_runtime_material_derives_cookie_and_bearer() {
    #[derive(Clone, Default)]
    struct ProbeState {
        seen: Arc<
            Mutex<
                Vec<(
                    String,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                )>,
            >,
        >,
    }

    async fn record_probe(
        State(state): State<ProbeState>,
        headers: AxumHeaderMap,
        body: String,
    ) -> Json<Value> {
        let is_challenge_probe = body.contains("\"ctype\":\"generation\"");
        state.seen.lock().unwrap().push((
            body,
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
            headers
                .get("cookie")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
            headers
                .get("device-id")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
            headers
                .get("referring-pathname")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
            headers
                .get("referring-origin")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
        ));
        if is_challenge_probe {
            Json(json!({ "required": false }))
        } else {
            Json(json!({ "ok": true }))
        }
    }

    let state = ProbeState::default();
    let app = Router::new()
        .route("/api/user/user_config/", post(record_probe))
        .route("/api/c/check", post(record_probe))
        .with_state(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("axum serve");
    });

    let redis_pool = deadpool_redis::Config::from_url("redis://localhost:6379")
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("redis pool");
    let http = Client::new();
    let session_token = "eyJhbGciOiJIUzI1NiJ9.eyJleHAiOjQxMDE4NDYwMDB9.signature";
    let cookie_header = format!(
        "__client=refresh-token-123; __session={session_token}; ajs_anonymous_id=device-123; other=1"
    );
    let response = ensure_credential_runtime(
        &redis_pool,
        None,
        &http,
        GatewayKeepaliveEnsureRequest {
            project_id: None,
            session_key: None,
            previous_response_id: None,
            credential_id: None,
            account_name: Some("suno-main".to_string()),
            provider_account_id: "provider-1".to_string(),
            adapter: "suno_compatible".to_string(),
            base_url: format!("http://{addr}"),
            model: "chirp-v3-5".to_string(),
            api_key: Some(cookie_header.clone()),
            headers: HashMap::from([(
                "Accept".to_string(),
                "application/json, text/plain, */*".to_string(),
            )]),
            extra_body: None,
            session_auth: Some(SessionAuthConfig {
                transport: "bearer".to_string(),
                primary_cookie_name: None,
                secondary_cookie_name: None,
                header_name: Some("authorization".to_string()),
                expires_at: None,
            }),
            expires_at: None,
            runtime_state_object_key: None,
        },
    )
    .await
    .expect("ensure suno runtime");

    assert!(response.ready);
    assert_eq!(response.api_key.as_deref(), Some(session_token));
    assert_eq!(
        response
            .headers
            .as_ref()
            .and_then(|headers| read_header_case_insensitive(headers, "cookie"))
            .as_deref(),
        Some(cookie_header.as_str())
    );
    assert_eq!(
        response
            .headers
            .as_ref()
            .and_then(|headers| read_header_case_insensitive(headers, "device-id"))
            .as_deref(),
        Some("device-123")
    );
    assert_eq!(
        response
            .headers
            .as_ref()
            .and_then(|headers| read_header_case_insensitive(headers, "referring-pathname"))
            .as_deref(),
        Some("/")
    );
    assert_eq!(
        response
            .headers
            .as_ref()
            .and_then(|headers| read_header_case_insensitive(headers, "referring-origin"))
            .as_deref(),
        Some("https://suno.com")
    );
    let derived_expiry = response.expires_at.clone();
    assert!(derived_expiry.is_some());
    assert_eq!(
        response
            .session_auth
            .as_ref()
            .and_then(|session_auth| session_auth.expires_at.clone()),
        derived_expiry
    );

    let seen = state.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 2);
    assert!(seen
        .iter()
        .all(|(_, auth, cookie, device_id, pathname, origin)| {
            auth.as_deref() == Some(&format!("Bearer {session_token}"))
                && cookie.as_deref() == Some(cookie_header.as_str())
                && device_id.as_deref() == Some("device-123")
                && pathname.as_deref() == Some("/")
                && origin.as_deref() == Some("https://suno.com")
        }));

    server.abort();
}

#[tokio::test]
async fn ensure_suno_runtime_material_allows_expired_session_token_if_probes_pass() {
    #[derive(Clone, Default)]
    struct ProbeState {
        seen: Arc<Mutex<Vec<String>>>,
    }

    async fn record_probe(State(state): State<ProbeState>, body: String) -> Json<Value> {
        state.seen.lock().unwrap().push(body.clone());
        if body.contains("\"ctype\":\"generation\"") {
            Json(json!({ "required": false }))
        } else {
            Json(json!({ "ok": true }))
        }
    }

    let state = ProbeState::default();
    let app = Router::new()
        .route("/api/user/user_config/", post(record_probe))
        .route("/api/c/check", post(record_probe))
        .with_state(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("axum serve");
    });

    let redis_pool = deadpool_redis::Config::from_url("redis://localhost:6379")
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("redis pool");
    let http = Client::new();
    let expired_session_token = "eyJhbGciOiJIUzI1NiJ9.eyJleHAiOjE3MDAwMDAwMDB9.signature";
    let cookie_header = format!(
        "__client=refresh-token-123; __session={expired_session_token}; ajs_anonymous_id=device-123; other=1"
    );
    let response = ensure_credential_runtime(
        &redis_pool,
        None,
        &http,
        GatewayKeepaliveEnsureRequest {
            project_id: None,
            session_key: None,
            previous_response_id: None,
            credential_id: None,
            account_name: Some("suno-main".to_string()),
            provider_account_id: "provider-1".to_string(),
            adapter: "suno_compatible".to_string(),
            base_url: format!("http://{addr}"),
            model: "chirp-v3-5".to_string(),
            api_key: Some(cookie_header),
            headers: HashMap::new(),
            extra_body: None,
            session_auth: Some(SessionAuthConfig {
                transport: "bearer".to_string(),
                primary_cookie_name: None,
                secondary_cookie_name: None,
                header_name: Some("authorization".to_string()),
                expires_at: None,
            }),
            expires_at: None,
            runtime_state_object_key: None,
        },
    )
    .await
    .expect("ensure suno runtime with expired token");

    assert!(response.ready);
    assert_eq!(state.seen.lock().unwrap().len(), 2);
    server.abort();
}
