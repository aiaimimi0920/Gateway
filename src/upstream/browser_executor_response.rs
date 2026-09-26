//! Bounded, secret-safe handling of remote browser executor responses.

use serde_json::Value;
use tracing::debug;

use crate::error::GatewayError;
use crate::protocol::upstream_body::collect_bounded_upstream_text;
use crate::upstream::browser_executor_helpers::{
    classify_browser_executor_invocation_failure, extract_browser_executor_invocation_success,
    parse_browser_executor_invocation_response_body,
};
use crate::upstream::request_time_browser_policy::{
    remote_browser_executor_required_failed_error, RequestTimeBrowserPolicy,
};

pub(crate) async fn decode_remote_browser_executor_response(
    response: rquest::Response,
    policy: RequestTimeBrowserPolicy,
    provider: &str,
    provider_account_id: &str,
    endpoint_kind: &str,
) -> Result<Option<Value>, GatewayError> {
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        debug!(
            provider,
            provider_account_id,
            endpoint_kind,
            status,
            local_fallback_allowed = !policy.forbids_local_fallback(),
            "remote browser executor returned non-success status"
        );
        return fallback_or_required_failure(
            policy,
            format!("Remote browser executor returned status {status}."),
        );
    }

    let body_text =
        match collect_bounded_upstream_text(response, "remote browser executor response").await {
            Ok(body_text) => body_text,
            Err(error) => {
                debug!(
                    provider,
                    provider_account_id,
                    endpoint_kind,
                    status,
                    response_error_code = error.code.as_deref().unwrap_or("unknown"),
                    local_fallback_allowed = !policy.forbids_local_fallback(),
                    "remote browser executor response could not be read safely"
                );
                return fallback_or_required_failure(
                    policy,
                    "Remote browser executor response could not be read safely.".to_string(),
                );
            }
        };

    let result = match parse_browser_executor_invocation_response_body(&body_text) {
        Ok(result) => result,
        Err(_) => {
            debug!(
                provider,
                provider_account_id,
                endpoint_kind,
                status,
                body_bytes = body_text.len(),
                local_fallback_allowed = !policy.forbids_local_fallback(),
                "remote browser executor returned invalid JSON"
            );
            return fallback_or_required_failure(
                policy,
                "Remote browser executor returned invalid JSON.".to_string(),
            );
        }
    };

    if result.ok {
        return Ok(extract_browser_executor_invocation_success(result));
    }

    Err(classify_browser_executor_invocation_failure(
        result, provider,
    ))
}

fn fallback_or_required_failure(
    policy: RequestTimeBrowserPolicy,
    message: String,
) -> Result<Option<Value>, GatewayError> {
    if policy.forbids_local_fallback() {
        return Err(remote_browser_executor_required_failed_error(message));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::*;
    use crate::protocol::upstream_body::MAX_ACCUMULATED_UPSTREAM_BODY_BYTES;

    fn single_response(
        status: u16,
        status_text: &'static str,
        declared_length: u64,
        body: &'static str,
    ) -> (String, std::thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind response server");
        let addr = listener.local_addr().expect("read response server addr");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);
            let response = format!(
                "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {declared_length}\r\n\r\n{body}"
            );
            let _ = stream.write_all(response.as_bytes());
        });
        (format!("http://{addr}"), handle)
    }

    async fn fetch_response(
        status: u16,
        status_text: &'static str,
        declared_length: u64,
        body: &'static str,
    ) -> (rquest::Response, std::thread::JoinHandle<()>) {
        let (url, server) = single_response(status, status_text, declared_length, body);
        let response = rquest::Client::builder()
            .build()
            .expect("build client")
            .get(url)
            .send()
            .await
            .expect("fetch response");
        (response, server)
    }

    #[tokio::test]
    async fn success_preserves_executor_result() {
        const BODY: &str = r#"{"ok":true,"result":{"answer":"ready"}}"#;
        let (response, server) = fetch_response(200, "OK", BODY.len() as u64, BODY).await;

        let result = decode_remote_browser_executor_response(
            response,
            RequestTimeBrowserPolicy::RemoteOnly,
            "producer",
            "account-1",
            "chat_completions",
        )
        .await
        .expect("valid response");
        server.join().expect("response server exits");

        assert_eq!(result, Some(serde_json::json!({"answer": "ready"})));
    }

    #[tokio::test]
    async fn non_success_does_not_expose_executor_body() {
        const BODY: &str = "api_key=executor-secret";
        let (response, server) =
            fetch_response(503, "Service Unavailable", BODY.len() as u64, BODY).await;

        let error = decode_remote_browser_executor_response(
            response,
            RequestTimeBrowserPolicy::RemoteOnly,
            "producer",
            "account-1",
            "chat_completions",
        )
        .await
        .expect_err("remote-only failure must fail closed");
        server.join().expect("response server exits");

        assert_eq!(
            error.code.as_deref(),
            Some("browser_executor_required_failed")
        );
        assert!(!error.message.contains("executor-secret"));
    }

    #[tokio::test]
    async fn oversized_success_response_fails_closed_without_allocating_body() {
        let declared_length = MAX_ACCUMULATED_UPSTREAM_BODY_BYTES as u64 + 1;
        let (response, server) = fetch_response(200, "OK", declared_length, "").await;

        let error = decode_remote_browser_executor_response(
            response,
            RequestTimeBrowserPolicy::RemoteOnly,
            "producer",
            "account-1",
            "chat_completions",
        )
        .await
        .expect_err("oversized remote-only response must fail closed");
        server.join().expect("response server exits");

        assert_eq!(
            error.code.as_deref(),
            Some("browser_executor_required_failed")
        );
        assert_eq!(
            error.message,
            "Remote browser executor response could not be read safely."
        );
    }

    #[tokio::test]
    async fn invalid_json_preserves_local_fallback_policy() {
        const BODY: &str = "not-json api_key=executor-secret";
        let (response, server) = fetch_response(200, "OK", BODY.len() as u64, BODY).await;

        let result = decode_remote_browser_executor_response(
            response,
            RequestTimeBrowserPolicy::LocalAllowed,
            "producer",
            "account-1",
            "chat_completions",
        )
        .await
        .expect("local fallback remains available");
        server.join().expect("response server exits");

        assert_eq!(result, None);
    }
}
