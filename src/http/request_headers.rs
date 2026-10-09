use axum::http::HeaderMap;

use crate::auth::adapter::AuthRequest;
use crate::config::Config;
use crate::console::ConsoleAuthRuntime;
use crate::error::GatewayError;
use crate::pipeline::PipelineContext;

const DEFAULT_INBOUND_API_KEY_HEADERS: &[&str] = &["x-api-key", "api-key", "x-goog-api-key"];
const DEFAULT_TRUSTED_FORWARDED_HEADERS: &[&str] =
    &["x-credential-ref", "x-neuro-user", "x-neuro-cred-source"];
const DEFAULT_TRACE_HEADERS: &[&str] = &["x-request-id", "traceparent", "tracestate"];
pub(crate) const INTERNAL_GATEWAY_HEADER_DENYLIST: &[&str] =
    &["x-neuro-account-group", "x-account-group-id"];
const INTERNAL_API_KEY_HEADER: &str = "x-internal-api-key";

pub fn apply_public_request_headers(
    ctx: &mut PipelineContext,
    headers: &HeaderMap,
    config: &Config,
    console_auth: &ConsoleAuthRuntime,
    extra_headers: &[&str],
) -> Result<(), GatewayError> {
    ctx.forced_upstream_protocol = super::debug_protocol::requested_target(headers)?;
    if let Some(api_key) = extract_inbound_api_key(headers, config) {
        ctx.request_headers.insert("x-api-key".to_string(), api_key);
    }

    let requested_account_group = trusted_account_group_value(headers);
    let management_token = header_value(headers, INTERNAL_API_KEY_HEADER);
    let trusted_forwarded_headers_allowed = match management_token.as_deref() {
        Some(token) => match console_auth.verify_management_token(token) {
            Ok(()) => true,
            Err(error) if requested_account_group.is_some() => return Err(error),
            Err(_) => false,
        },
        None if requested_account_group.is_some() => {
            console_auth.verify_management_token("")?;
            unreachable!("empty management token verification must fail")
        }
        None => false,
    };

    if trusted_forwarded_headers_allowed {
        for header_name in DEFAULT_TRUSTED_FORWARDED_HEADERS {
            if let Some(value) = header_value(headers, header_name) {
                ctx.request_headers
                    .insert(header_name.to_ascii_lowercase(), value);
            }
        }
        if let Some(account_group) = requested_account_group {
            ctx.account_group_id = Some(account_group);
        }
    }

    for header_name in DEFAULT_TRACE_HEADERS {
        if let Some(value) = trace_header_value(headers, header_name) {
            ctx.request_headers
                .insert(header_name.to_ascii_lowercase(), value);
        }
    }

    for header_name in extra_headers {
        if is_internal_gateway_header(header_name) {
            continue;
        }
        if let Some(value) = header_value(headers, header_name) {
            ctx.request_headers
                .insert(header_name.to_ascii_lowercase(), value);
        }
    }

    Ok(())
}

pub fn build_public_auth_request(
    config: &Config,
    bearer_token: Option<&str>,
    headers: &HeaderMap,
    path: &str,
    method: &str,
) -> AuthRequest {
    AuthRequest {
        authorization: bearer_token.map(|value| format!("Bearer {value}")),
        api_key: extract_inbound_api_key(headers, config),
        path: path.to_string(),
        method: method.to_string(),
    }
}

pub fn extract_inbound_api_key(headers: &HeaderMap, config: &Config) -> Option<String> {
    for header_name in DEFAULT_INBOUND_API_KEY_HEADERS.iter().copied().chain(
        config
            .gateway_inbound_api_key_header_aliases
            .iter()
            .map(String::as_str),
    ) {
        if let Some(value) = header_value(headers, header_name) {
            return Some(value);
        }
    }

    None
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn trusted_account_group_value(headers: &HeaderMap) -> Option<String> {
    for header_name in INTERNAL_GATEWAY_HEADER_DENYLIST {
        if let Some(value) = header_value(headers, header_name) {
            return Some(value);
        }
    }
    None
}

pub(crate) fn is_internal_gateway_header(name: &str) -> bool {
    name.eq_ignore_ascii_case(super::debug_protocol::HEADER)
        || INTERNAL_GATEWAY_HEADER_DENYLIST
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(name))
}

#[cfg(test)]
pub(crate) struct TestConsoleAuthFixture {
    root: std::path::PathBuf,
    pub(crate) runtime: ConsoleAuthRuntime,
}

#[cfg(test)]
impl TestConsoleAuthFixture {
    pub(crate) fn with_environment_token(token: &str) -> Self {
        Self::new(Some(token), None)
    }

    fn with_bootstrapped_token(token: &str) -> Self {
        Self::new(None, Some(token))
    }

    pub(crate) fn without_management_token() -> Self {
        Self::new(None, None)
    }

    fn new(environment_token: Option<&str>, bootstrap_token: Option<&str>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gateway-request-header-auth-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let routes_file = root.join("routes.yaml");
        std::fs::write(&routes_file, b"providers: []\n").unwrap();
        let console =
            crate::console::ConsoleConfig::from_values(crate::console::ConsoleConfigValues {
                state_dir: Some(root.join("state")),
                routes_file: Some(routes_file),
                ..crate::console::ConsoleConfigValues::default()
            })
            .unwrap();
        let runtime =
            ConsoleAuthRuntime::new(&console, environment_token.map(str::to_string)).unwrap();
        if let Some(token) = bootstrap_token {
            runtime
                .bootstrap(&crate::console::ConsoleRequestContext::loopback(), token)
                .unwrap();
        }
        Self { root, runtime }
    }
}

#[cfg(test)]
impl Drop for TestConsoleAuthFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[cfg(test)]
mod tests;

fn trace_header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    header_value(headers, name).filter(|value| value.len() <= 512 && value.is_ascii())
}
