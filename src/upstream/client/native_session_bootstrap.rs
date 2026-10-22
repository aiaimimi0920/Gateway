//! Single-flight SDK settings bootstrap, after auth/filter/route and credential refresh.
use super::*;
use std::collections::HashMap;

fn bootstrap_error(message: &str) -> GatewayError {
    let mut error = GatewayError::server_error(message);
    error.http_status = Some(502);
    // A failed initialization may be retried by a subsequent request, not by
    // silently replaying this request or switching credentials under one scope.
    error.retryable = false;
    error
}

fn validate_settings(status: u16, bytes: &[u8]) -> Result<(), GatewayError> {
    if !(200..300).contains(&status) {
        return Err(bootstrap_error("Native settings initialization failed"));
    }
    if !serde_json::from_slice::<Value>(bytes)
        .ok()
        .is_some_and(|v| v.is_object())
    {
        return Err(bootstrap_error("Native settings returned invalid JSON"));
    }
    Ok(())
}

impl UpstreamClient {
    pub(crate) async fn initialize_sdk_session(
        &self,
        builder: &reqwest::RequestBuilder,
        payload: &ProviderAccountPayload,
        scope: Scope,
        connection: &NativeCodexConnection,
        inbound: Option<&HashMap<String, String>>,
    ) -> Result<Option<std::sync::Arc<tokio::sync::OnceCell<()>>>, GatewayError> {
        if inbound
            .and_then(|h| h.get("originator").zip(h.get("user-agent")))
            .is_some_and(|(origin, ua)| crate::protocol::codex_wire::native_identity(origin, ua))
        {
            return Ok(None);
        }
        let request = builder
            .try_clone()
            .ok_or_else(|| bootstrap_error("Non-replayable native request"))?
            .build()
            .map_err(|_| bootstrap_error("Invalid native request"))?;
        let binding = binding::key(payload, &request)?;
        let gate = self
            .native_sessions
            .initialization(scope, binding, request.url())
            .map_err(GatewayError::bad_request)?;
        // This Arc pins the exact registry entry during initialization and waiting.
        // OnceCell retries after failure/cancellation, including successful responses
        // without Set-Cookie; Cookie presence never substitutes for initialization.
        let ready = tokio::time::timeout(
            Duration::from_secs(12),
            gate.get_or_try_init(|| async {
                let mut headers = HeaderMap::new();
                for name in ["user-agent", "authorization", "chatgpt-account-id"] {
                    if let Some(value) = request.headers().get(name) {
                        headers.insert(name, value.clone());
                    }
                }
                headers.insert(
                    "cache-control",
                    HeaderValue::from_static("no-cache, no-store"),
                );
                headers.insert("accept", HeaderValue::from_static("*/*"));
                let (status, bytes) = self
                    .native_user_settings(payload, scope, connection, &headers)
                    .await?;
                validate_settings(status, &bytes)
            }),
        )
        .await
        .map_err(|_| bootstrap_error("Native settings initialization timed out"))?;
        ready.map(|_| ()).map_err(|mut error| {
            error.retryable = false;
            error
        })?;
        // Keep this lease through the following POST and Cookie ingestion. A
        // suspended task must not send using a newly recreated, uninitialized jar.
        Ok(Some(gate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_session_bootstrap_only_accepts_successful_complete_settings() {
        assert!(validate_settings(200, b"{}").is_ok());
        for status in [302, 401, 403, 429, 500, 503] {
            assert!(validate_settings(status, b"{}").is_err());
        }
        for body in [b"".as_slice(), b"null", b"[]", b"{", b"not-json"] {
            assert!(validate_settings(200, body).is_err());
        }
        assert!(!bootstrap_error("fixture").retryable);
    }
}
