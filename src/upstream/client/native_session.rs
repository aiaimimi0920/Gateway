//! Opt-in native session transport: selected provider auth, no redirects or Cookie export.
use super::*;
use crate::http_client::{native_cookie_sessions::Scope, NativeCodexConnection};
use crate::upstream::headers::build_upstream_headers;
#[path = "native_session_binding.rs"]
mod binding;
#[path = "native_session_bootstrap.rs"]
mod bootstrap;

pub(crate) fn eligible(payload: &ProviderAccountPayload) -> bool {
    let selected = build_upstream_headers(payload);
    payload.adapter == "openai_compatible"
        && payload.base_url.trim_end_matches('/') == "https://chatgpt.com/backend-api/codex"
        && payload.request_client_profile
            == Some(crate::protocol::request_profile::RequestClientProfile::CodexCli)
        && payload.resolve_execution_mode(EndpointKind::Responses)
            == ProviderExecutionMode::DirectHttp
        && selected
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .is_some_and(|v| !v.is_empty())
        && selected
            .get("chatgpt-account-id")
            .is_some_and(|v| !v.as_bytes().is_empty())
        && !selected.contains_key("api-key")
        && !selected.contains_key("x-api-key")
}

impl UpstreamClient {
    pub(super) async fn send_native_session(
        &self,
        mut builder: reqwest::RequestBuilder,
        payload: &ProviderAccountPayload,
        scope: Scope,
    ) -> Result<reqwest::Response, GatewayError> {
        let request = builder
            .try_clone()
            .ok_or_else(|| GatewayError::bad_request("Non-replayable native session request"))?
            .build()
            .map_err(|_| GatewayError::bad_request("Invalid native session request"))?;
        let binding = binding::key(payload, &request)?;
        let url = request.url();
        let cookie = self
            .native_sessions
            .prepare(scope, binding, url)
            .map_err(GatewayError::bad_request)?;
        // Explicit operator Cookie wins; inbound Cookie is never copied here.
        if !request.headers().contains_key("cookie") {
            if let Some(cookie) = cookie {
                builder = builder.header("cookie", cookie);
            }
        }
        let response = builder
            .send()
            .await
            .map_err(|e| crate::error::classify_native_network_error(e, &payload.adapter))?;
        let values: Vec<_> = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .take(32)
            .filter_map(|v| v.to_str().ok())
            .collect();
        self.native_sessions
            .receive(scope, binding, response.url(), &values);
        Ok(response)
    }

    pub(crate) async fn native_user_settings(
        &self,
        payload: &ProviderAccountPayload,
        scope: Scope,
        connection: &NativeCodexConnection,
        inbound: &HeaderMap,
    ) -> Result<(u16, Vec<u8>), GatewayError> {
        let selected = build_upstream_headers(payload);
        let mut headers = HeaderMap::new();
        // The CLI settings client has a different header contract from Responses.
        // Retain its actual ordering but replace all authentication/account material.
        for (name, value) in inbound {
            match name.as_str() {
                "authorization" | "chatgpt-account-id" => {
                    if let Some(selected) = selected.get(name) {
                        headers.insert(name.clone(), selected.clone());
                    }
                }
                "user-agent" | "accept" | "cache-control" if value.as_bytes().len() <= 1024 => {
                    headers.insert(name.clone(), value.clone());
                }
                _ => {}
            }
        }
        for name in ["authorization", "chatgpt-account-id", "cookie"] {
            if !headers.contains_key(name) {
                if let Some(value) = selected.get(name) {
                    headers.insert(name, value.clone());
                }
            }
        }
        // Explicit operator headers have the same precedence as Responses.
        for name in payload.headers.keys() {
            if let Some(value) = selected.get(name) {
                if let Ok(name) = HeaderName::from_bytes(name.as_bytes()) {
                    headers.insert(name, value.clone());
                }
            }
        }
        let builder = connection
            .session_client(self.timeout)
            .get("https://chatgpt.com/backend-api/wham/settings/user")
            .headers(headers)
            .timeout(std::time::Duration::from_secs(10));
        let response = self.send_native_session(builder, payload, scope).await?;
        let status = response.status().as_u16();
        let mut stream = response.bytes_stream();
        let mut body = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| crate::error::classify_native_network_error(e, &payload.adapter))?;
            if chunk.len() > (1024usize * 1024).saturating_sub(body.len()) {
                return Err(GatewayError::server_error(
                    "Native settings response exceeds 1 MiB",
                ));
            }
            body.extend_from_slice(&chunk);
        }
        Ok((status, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_session_only_admits_bearer_account_credentials() {
        let mut payload: ProviderAccountPayload = serde_json::from_value(serde_json::json!({
            "adapter":"openai_compatible", "base_url":"https://chatgpt.com/backend-api/codex",
            "api_key":"synthetic", "headers":{"chatgpt-account-id":"account"},
            "request_client_profile":"codex_cli"
        }))
        .unwrap();
        assert!(eligible(&payload));
        payload.auth_mode = Some("api-key".into());
        assert!(!eligible(&payload));
        payload.auth_mode = Some("x-api-key".into());
        assert!(!eligible(&payload));
        payload.auth_mode = None;
        payload.headers.clear();
        assert!(!eligible(&payload));
    }
}
