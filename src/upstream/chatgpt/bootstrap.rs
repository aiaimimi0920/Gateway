use std::collections::HashMap;
use std::time::Duration;

use rquest::{Client, Method};

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::chatgpt::web_reverse as surface;
use crate::protocol::upstream_body::collect_bounded_upstream_charset_text_with_provider;
use crate::routing::candidate::ProviderAccountPayload;

use super::headers::build_bootstrap_headers;
use super::web_reverse::{
    build_target_url, merge_bootstrap_from_fallback, ChatGptWebRequestContext,
};
use super::PROVIDER;

pub async fn bootstrap_site(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    request_context: &ChatGptWebRequestContext,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<surface::ChatGptWebBootstrap, GatewayError> {
    let fallback = surface::bootstrap_from_payload_cache(payload.extra_body.as_ref());
    if let Some(cached) = fallback.as_ref() {
        return Ok(merge_bootstrap_from_fallback(cached.clone(), None));
    }

    let bootstrap_url =
        build_target_url(request_context, surface::CHATGPT_WEB_DEFAULT_BOOTSTRAP_PATH);
    let headers = build_bootstrap_headers(request_context, payload, extra_headers);
    let response = http
        .request(Method::GET, &bootstrap_url)
        .headers(headers)
        .timeout(timeout.max(Duration::from_secs(30)))
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_text = collect_bounded_upstream_charset_text_with_provider(
        response,
        "ChatGPT Web reverse bootstrap body",
        PROVIDER,
    )
    .await?;
    if !(200..300).contains(&status)
        || surface::response_indicates_browser_challenge(
            status,
            content_type.as_deref(),
            &body_text,
        )
        || surface::response_indicates_session_invalid(status, content_type.as_deref(), &body_text)
    {
        return Err(surface::classify_chatgpt_web_http_error(
            status,
            content_type.as_deref(),
            &body_text,
        ));
    }

    let bootstrap = surface::parse_bootstrap_from_html(&body_text)
        .or_else(|primary_error| fallback.clone().ok_or(primary_error))?;
    Ok(merge_bootstrap_from_fallback(bootstrap, fallback.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use serde_json::json;
    use tokio::net::TcpListener;

    use crate::upstream::chatgpt::web_reverse::build_request_context;

    fn make_payload(
        base_url: &str,
        extra_body: Option<HashMap<String, serde_json::Value>>,
    ) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: "chatgpt_web_reverse_compatible".to_string(),
            base_url: base_url.to_string(),
            api_key: "session-token".to_string(),
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
            extra_body,
            session_auth: None,
            keepalive: None,
        }
    }

    async fn spawn_bootstrap_server(
        status: u16,
        content_type: &'static str,
        body: &'static str,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("addr");
        let app = Router::new().route(
            "/",
            get(move || async move {
                (
                    axum::http::StatusCode::from_u16(status).expect("status"),
                    [(axum::http::header::CONTENT_TYPE, content_type)],
                    body,
                )
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });
        format!("http://{}", address)
    }

    #[tokio::test]
    async fn bootstrap_site_parses_html_bootstrap_material() {
        let base_url = spawn_bootstrap_server(
            200,
            "text/html; charset=utf-8",
            r#"<html data-build="build-123"><script src="https://chatgpt.com/c/foo/_/bar.js"></script></html>"#,
        )
        .await;
        let payload = make_payload(&base_url, None);
        let request_context = build_request_context(&payload);
        let bootstrap = bootstrap_site(
            &Client::new(),
            Duration::from_secs(5),
            &payload,
            &request_context,
            None,
        )
        .await
        .expect("bootstrap");

        assert_eq!(bootstrap.pow_script_sources.len(), 1);
        assert_eq!(bootstrap.pow_data_build.as_deref(), Some("c/foo/_"));
    }

    #[tokio::test]
    async fn bootstrap_site_merges_payload_cache_fallback() {
        let base_url =
            spawn_bootstrap_server(200, "text/html; charset=utf-8", "<html></html>").await;
        let mut extra_body = HashMap::new();
        extra_body.insert(
            "chatgptPowSources".to_string(),
            json!(["https://chatgpt.com/backend-api/sentinel/sdk.js"]),
        );
        extra_body.insert("chatgptPowDataBuild".to_string(), json!("build-from-cache"));
        let payload = make_payload(&base_url, Some(extra_body));
        let request_context = build_request_context(&payload);
        let bootstrap = bootstrap_site(
            &Client::new(),
            Duration::from_secs(5),
            &payload,
            &request_context,
            None,
        )
        .await
        .expect("bootstrap");

        assert_eq!(
            bootstrap.pow_data_build.as_deref(),
            Some("build-from-cache")
        );
        assert!(!bootstrap.pow_script_sources.is_empty());
    }

    #[tokio::test]
    async fn bootstrap_site_prefers_payload_cache_over_live_challenge() {
        let base_url = spawn_bootstrap_server(
            403,
            "text/html; charset=utf-8",
            "<!doctype html><html><body>Just a moment... verify you are human</body></html>",
        )
        .await;
        let mut extra_body = HashMap::new();
        extra_body.insert(
            "chatgptPowSources".to_string(),
            json!(["https://chatgpt.com/backend-api/sentinel/sdk.js"]),
        );
        extra_body.insert("chatgptPowDataBuild".to_string(), json!("build-from-cache"));
        let payload = make_payload(&base_url, Some(extra_body));
        let request_context = build_request_context(&payload);
        let bootstrap = bootstrap_site(
            &Client::new(),
            Duration::from_secs(5),
            &payload,
            &request_context,
            None,
        )
        .await
        .expect("cached bootstrap should avoid live challenge");

        assert_eq!(
            bootstrap.pow_data_build.as_deref(),
            Some("build-from-cache")
        );
    }

    #[tokio::test]
    async fn bootstrap_site_classifies_browser_challenge_html() {
        let base_url = spawn_bootstrap_server(
            403,
            "text/html; charset=utf-8",
            "<!doctype html><html><body>Just a moment... verify you are human</body></html>",
        )
        .await;
        let payload = make_payload(&base_url, None);
        let request_context = build_request_context(&payload);
        let error = bootstrap_site(
            &Client::new(),
            Duration::from_secs(5),
            &payload,
            &request_context,
            None,
        )
        .await
        .expect_err("challenge");

        assert_eq!(
            error.code.as_deref(),
            Some(surface::CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE)
        );
    }

    #[tokio::test]
    async fn bootstrap_site_classifies_session_invalid_html() {
        let base_url = spawn_bootstrap_server(
            401,
            "text/html; charset=utf-8",
            "<!doctype html><html><body>session expired, please sign in</body></html>",
        )
        .await;
        let payload = make_payload(&base_url, None);
        let request_context = build_request_context(&payload);
        let error = bootstrap_site(
            &Client::new(),
            Duration::from_secs(5),
            &payload,
            &request_context,
            None,
        )
        .await
        .expect_err("session invalid");

        assert_eq!(
            error.code.as_deref(),
            Some(surface::CHATGPT_WEB_SESSION_INVALID_CODE)
        );
    }
}
