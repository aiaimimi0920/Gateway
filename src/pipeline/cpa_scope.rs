//! Bind identity only after authentication, without placing private data in the wire request.
use super::*;
use crate::upstream::request_identity_context::{self as context, Context};
use sha2::{Digest, Sha256};

pub(super) async fn send(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
) -> Result<PipelineOutput, GatewayError> {
    let mut identity = Context::default();
    if let Some(session) = &ctx.session {
        identity.requested_betas = ctx
            .request_headers
            .get("anthropic-beta")
            .filter(|v| v.len() <= 8192)
            .cloned()
            .unwrap_or_default();
        // Follow AuthRequest::credential precedence; no bearer/token is retained in the context.
        if session.api_key_id.as_deref() != Some(crate::auth::session::DEV_MODE_API_KEY_ID) {
            identity.caller_scope = credential_scope(
                ctx.bearer_token.as_deref(),
                ctx.request_headers.get("x-api-key").map(String::as_str),
            );
        }
        identity.native_claude = ctx
            .request_headers
            .get("user-agent")
            .zip(ctx.request_headers.get("x-app"))
            .is_some_and(|(ua, app)| crate::protocol::claude_wire::identity(ua, app));
        identity.native_claude &= ctx
            .request_headers
            .get("anthropic-beta")
            .is_some_and(|value| value.split(',').any(|v| v.trim() == "claude-code-20250219"))
            && gateway_cpa_claude_profile::session_header(&ctx.canonical_req.raw_body).is_some();
        // Session labels are caller-controlled correlation hints, not authentication claims.
        // Claude-only hints are accepted only on the matching native client path.
        let keys = [
            ("session-id", "codex"),
            ("session_id", "codex"),
            ("x-http-session-id", "agy"),
            ("x-session-id", "header"),
            ("x-session-affinity", "affinity"),
            ("x-client-request-id", "clientreq"),
        ];
        identity.session_hint = keys.iter().find_map(|(key, prefix)| {
            ctx.client_session_hints
                .get(*key)
                .or_else(|| ctx.request_headers.get(*key))
                .filter(|value| valid(value))
                .map(|value| format!("{prefix}:{}", value.trim()))
        });
        if identity.native_claude {
            identity.native_os = ctx
                .request_headers
                .get("x-stainless-os")
                .filter(|v| valid(v))
                .cloned();
            identity.native_arch = ctx
                .request_headers
                .get("x-stainless-arch")
                .filter(|v| valid(v))
                .cloned();
            if let Some(value) = ctx
                .request_headers
                .get("x-claude-code-session-id")
                .filter(|v| valid(v))
            {
                identity.session_hint = Some(format!("claude:{}", value.trim()));
            }
        }
    }
    context::within(identity, super::stage_send::run(ctx, state)).await
}

fn valid(value: &str) -> bool {
    !value.chars().any(char::is_control) && !value.trim().is_empty() && value.trim().len() <= 256
}

fn credential_scope(bearer: Option<&str>, api_key: Option<&str>) -> String {
    let request = crate::auth::adapter::AuthRequest {
        authorization: bearer.map(|key| format!("Bearer {key}")),
        api_key: api_key.map(str::to_owned),
        path: String::new(),
        method: String::new(),
    };
    request
        .credential()
        .filter(|key| !key.trim().is_empty())
        .map(|key| {
            format!(
                "{:x}",
                Sha256::digest(format!("cli-proxy-api:caller-scope:v1\0{}", key.trim()))
            )
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpa_scope_uses_the_credential_actually_selected_by_authentication() {
        assert_eq!(
            credential_scope(Some("untrusted-bearer"), Some("selected-key")),
            credential_scope(None, Some("selected-key"))
        );
        assert_ne!(
            credential_scope(Some("same-bearer"), Some("a")),
            credential_scope(Some("same-bearer"), Some("b"))
        );
        assert_eq!(
            credential_scope(Some("selected-key"), None),
            credential_scope(None, Some("selected-key"))
        );
    }
}
