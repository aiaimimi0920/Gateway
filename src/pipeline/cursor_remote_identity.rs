//! One frozen remote account. Tokens never enter generic candidates or HTTP pools.
use super::{CredentialSource, PipelineContext};
use crate::{
    error::GatewayError,
    routing::{
        candidate::{ProviderExecutionMode, RouteCandidate},
        config::{cursor, cursor_remote, RouteConfigSnapshot},
    },
    state::AppState,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub(super) struct Identity {
    state: Arc<AppState>,
    snapshot: Arc<RouteConfigSnapshot>,
    key: String,
    bearer: String,
    group: Option<String>,
    budget: super::request_budget::RequestBudget,
    cancel: Option<tokio::sync::watch::Receiver<bool>>,
    connection: Option<uuid::Uuid>,
    scope: &'static str,
    pub provider: String,
    pub credential: String,
}

pub(super) fn configured(_state: &AppState, snapshot: &RouteConfigSnapshot) -> bool {
    // Dependency discovery must not opt an unrelated configuration into this lane.
    // An imported remote provider still owns its errors when dependencies are absent.
    snapshot
        .document()
        .providers
        .iter()
        .any(cursor_remote::is_provider)
}

pub(super) async fn select(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
) -> Result<(), GatewayError> {
    let identity = Identity::bind(ctx, state).await?;
    ctx.candidates = vec![identity.candidate()?];
    Ok(())
}

impl Identity {
    pub(super) async fn bind(
        ctx: &PipelineContext,
        state: &Arc<AppState>,
    ) -> Result<Self, GatewayError> {
        if ctx.forced_upstream_protocol.is_some() {
            return Err(cursor::stop(
                GatewayError::bad_request("Cursor remote cannot use a forced upstream protocol.")
                    .with_code("debug_upstream_protocol_unavailable"),
            ));
        }
        if ctx.native_wire.as_ref().is_some_and(|wire| wire.compact) {
            return Err(cursor::stop(
                GatewayError::bad_request("Cursor remote does not support Responses compaction")
                    .with_code("cursor_remote_compact_unsupported"),
            ));
        }
        if state.pg_pool.is_some()
            || state.local_runtime.is_none()
            || ctx.credential_source != CredentialSource::Platform
            || ctx.credential_ref.is_some()
            || ctx.request_headers.contains_key("x-credential-ref")
            || ctx
                .session
                .as_ref()
                .and_then(|s| s.access_key_kind.as_deref())
                == Some("auto_route")
        {
            return Err(denied());
        }
        state.lifecycle.cursor_remote.ready()?;
        crate::protocol::cursor_remote::ingress::validate_query(
            &ctx.canonical_req,
            &ctx.query_params,
        )?;
        crate::protocol::cursor_remote::ingress::parse_with_continuation(
            &ctx.canonical_req,
            ctx.cursor_remote_continuation.as_ref(),
        )?;
        let scope = crate::protocol::cursor_remote::ingress::scope(&ctx.canonical_req)?;
        let snapshot = ctx.cash_route_snapshot.clone().ok_or_else(denied)?;
        let mut allowed = Vec::new();
        for p in snapshot
            .document()
            .providers
            .iter()
            .filter(|p| cursor_remote::is_provider(p))
        {
            for c in p.credentials.iter().filter(|c| c.enabled == Some(true)) {
                let identity = Self {
                    state: state.clone(),
                    snapshot: snapshot.clone(),
                    key: crate::local_runtime::access_keys::local_key_id(
                        state,
                        ctx.session.as_ref(),
                    )
                    .ok_or_else(denied)?
                    .into(),
                    bearer: ctx.bearer_token.clone().ok_or_else(denied)?,
                    group: ctx.account_group_id.clone(),
                    budget: ctx.request_budget.clone(),
                    cancel: ctx.caller_cancel.clone(),
                    connection: ctx.cursor_remote_connection,
                    scope,
                    provider: p.id.clone(),
                    credential: c.id.clone().ok_or_else(denied)?,
                };
                if identity.authorize().await.is_ok() {
                    allowed.push(identity);
                }
            }
        }
        // No pooling, ambiguity, account failover, or retry after a Free check fails.
        if allowed.len() != 1 {
            return Err(denied());
        }
        Ok(allowed.remove(0))
    }

    fn live(&self) -> bool {
        !self.state.shutdown.is_requested()
            && !self.state.lifecycle.is_draining()
            && self.state.route_config.snapshot().revision() == self.snapshot.revision()
            && !self
                .cancel
                .as_ref()
                .is_some_and(|rx| *rx.borrow() || rx.has_changed().is_err())
            && self.budget.check_inflight().is_ok()
    }
    pub(super) async fn authorize(&self) -> Result<(), GatewayError> {
        if !self.live() {
            return Err(denied());
        }
        let local = self.state.local_runtime.as_ref().ok_or_else(denied)?;
        let metadata = local
            .authorize_cursor_remote_account(
                &self.key,
                &self.bearer,
                cursor::MODEL,
                &self.provider,
                self.scope,
            )
            .await
            .map_err(cursor::stop)?;
        let mut groups = self
            .snapshot
            .account_group_constraint(self.group.as_deref())
            .map_err(|_| denied())?;
        if let Some(ids) =
            crate::access_key_groups::group_ids(Some(&metadata)).map_err(|_| denied())?
        {
            groups.intersect(&self.snapshot.access_key_group_constraint(&ids));
        }
        if !groups.allows_account_id(&self.credential) || !self.live() {
            return Err(denied());
        }
        self.input("")?;
        Ok(())
    }

    pub(super) fn input(&self, prompt: &str) -> Result<serde_json::Value, GatewayError> {
        let p = self
            .snapshot
            .document()
            .providers
            .iter()
            .find(|p| p.id == self.provider)
            .ok_or_else(denied)?;
        let c = p
            .credentials
            .iter()
            .find(|c| c.id.as_deref() == Some(&self.credential) && c.enabled == Some(true))
            .ok_or_else(denied)?;
        let token = c.auth_token.as_deref().ok_or_else(denied)?;
        let claims = crate::protocol::cursor_auth::claims(token).map_err(|_| denied())?;
        crate::protocol::cursor_auth::require_fresh(claims.exp).map_err(|_| denied())?;
        let email = crate::protocol::cursor_auth::normalize_email(
            c.account_name.as_deref().ok_or_else(denied)?,
        )
        .map_err(|_| denied())?;
        Ok(
            serde_json::json!({"token":token, "emailHash":format!("{:x}",Sha256::digest(email.as_bytes())),
            "prompt":prompt}),
        )
    }
    pub(super) fn binding(&self) -> String {
        // No bearer/token is retained in the continuation index.
        let mut binding = format!(
            "{}\n{}\n{}\n{}\n{}",
            self.key,
            self.provider,
            self.credential,
            self.snapshot.revision().id(),
            self.scope
        );
        // Full-history HTTP and individual sockets must not claim one another's Run.
        if let Some(connection) = self.connection {
            binding.push_str(&format!("\nws:{connection}"));
        }
        binding
    }
    pub(super) fn candidate(&self) -> Result<RouteCandidate, GatewayError> {
        let providers = self.snapshot.get_providers();
        let p = providers
            .iter()
            .find(|p| p.id == self.provider)
            .ok_or_else(denied)?;
        let c = p
            .credential_pool
            .iter()
            .find(|c| c.id == self.credential)
            .ok_or_else(denied)?;
        Ok(RouteCandidate {
            provider_account_id: p.id.clone(),
            provider_credential_id: Some(c.id.clone()),
            label: p.label.clone(),
            payload: c.payload.clone(),
            protocol_family: cursor_remote::AUTH_MODE.into(),
            protocol_profile: cursor_remote::AUTH_MODE.into(),
            supported_protocol_families: vec![],
            adapter: cursor_remote::AUTH_MODE.into(),
            model_alias: None,
            upstream_model: None,
            resolved_execution_mode: ProviderExecutionMode::DirectHttp,
            priority: 0,
            weight: 0,
            failure_count: 0,
            cooldown_until: None,
            routing_score: None,
            routing_health_weight: None,
            routing_capacity_weight: None,
            routing_degraded: None,
            routing_breaker_open: None,
            routing_degradation_reasons: vec![],
        })
    }
}
fn denied() -> GatewayError {
    crate::protocol::cursor_remote::error("account_not_authorized")
}
