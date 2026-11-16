//! Bind permanent grant admission to the exact pinned transport material before any network send.
use super::{accio, dim, grok, kiro, lobsterai, nous, transport, vertex, PendingRefresh};
use crate::console::{refresh_grants::*, RouteConfigRedisRevision, RouteConfigRuntime};
use crate::routing::{candidate::ProviderAccountPayload, config::*};

pub(super) enum Prepared {
    Generic(rquest::Request),
    Grok(grok::Prepared),
    Accio(accio::Prepared),
    Kiro(kiro::Prepared),
    Vertex(vertex::Prepared),
    Dim(dim::Prepared),
    Nous(nous::Prepared),
    Lobster(lobsterai::Prepared),
}

pub(super) enum Outcome {
    Token(transport::TokenResponse),
    Nous(crate::protocol::nous_portal::ReceivedTokens),
    Lobster(crate::protocol::lobsterai::ReceivedTokens),
}

impl Prepared {
    pub(super) async fn new(
        http: &rquest::Client,
        attempt: &RefreshAttempt,
        payload: &ProviderAccountPayload,
    ) -> Result<Self, transport::RefreshError> {
        if lobsterai::owns(attempt, payload) {
            return lobsterai::prepare(attempt, payload).map(Self::Lobster);
        }
        if nous::owns(attempt, payload) {
            return nous::prepare(attempt, payload).map(Self::Nous);
        }
        if dim::owns(attempt, payload) {
            return dim::prepare(attempt, payload).map(Self::Dim);
        }
        if vertex::owns(attempt, payload) {
            return vertex::prepare(attempt, payload).map(Self::Vertex);
        }
        if kiro::owns(attempt, payload) {
            return kiro::prepare(attempt, payload).map(Self::Kiro);
        }
        if accio::owns(attempt, payload) {
            return accio::prepare(attempt, payload).map(Self::Accio);
        }
        if grok::owns(attempt, payload) {
            grok::prepare(attempt, payload).map(Self::Grok)
        } else {
            transport::prepare(
                http,
                &attempt.endpoint,
                &attempt.refresh_token,
                &attempt.client_id,
                payload.adapter == crate::protocol::antigravity::ADAPTER,
            )
            .await
            .map(Self::Generic)
        }
    }

    pub(super) async fn execute(
        self,
        http: &rquest::Client,
    ) -> Result<Outcome, transport::RefreshError> {
        match self {
            Self::Generic(request) => transport::execute(http, request).await.map(Outcome::Token),
            Self::Grok(prepared) => prepared.execute(http).await.map(Outcome::Token),
            Self::Accio(prepared) => prepared.execute(http).await.map(Outcome::Token),
            Self::Kiro(prepared) => prepared.execute(http).await.map(Outcome::Token),
            Self::Vertex(prepared) => prepared.execute(http).await.map(Outcome::Token),
            Self::Dim(prepared) => prepared.execute(http).await.map(Outcome::Token),
            Self::Nous(prepared) => prepared.execute(http).await.map(Outcome::Nous),
            Self::Lobster(prepared) => prepared.execute().await.map(Outcome::Lobster),
        }
    }
}

pub(super) async fn reserve(
    runtime: &RouteConfigRuntime,
    snapshot: &RouteConfigSnapshot,
    provider: &str,
    credential: &str,
    attempt: &RefreshAttempt,
) -> Option<(GrantRequest, GrantReservation)> {
    let revision =
        RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
            .ok()?;
    let request = GrantRequest::from_revision(&revision, provider, credential).ok()?;
    let material = &request.material;
    if url::Url::parse(&attempt.endpoint).ok()?.as_str() != material.endpoint
        || attempt.client_id != material.client
        || attempt.access_token != material.access
        || attempt.refresh_token != material.refresh
        || attempt.context != material.context
        || attempt.kiro_oauth != material.kiro_oauth
        || attempt.vertex_oauth != material.vertex_oauth
        || attempt.dim_oauth != material.dim_oauth
        || attempt.nous_portal != material.nous_portal
        || attempt.lobsterai != material.lobsterai
        || attempt.accio_sgcookie != material.accio_sgcookie
        || !material.matches_expiry(attempt.expires_at.as_deref())
    {
        return None;
    }
    let reserved = match tokio::time::timeout(
        super::persistence::authority_budget(),
        runtime.reserve_refresh_grant(&revision, &request),
    )
    .await
    {
        Ok(Ok(reserved)) => reserved,
        _ => {
            tracing::warn!(
                provider,
                credential,
                "OAuth grant reservation unavailable or unacknowledged; not sending"
            );
            return None;
        }
    };
    Some((request, reserved))
}

pub(super) fn replay(
    record: &GrantRecord,
    request: &GrantRequest,
) -> Option<(transport::TokenResponse, String)> {
    let result = record.replay_for(request)?;
    Some((
        transport::TokenResponse {
            access_token: result.material.access.clone(),
            refresh_token: Some(result.material.refresh.clone()),
            expires_in: result.expires_in,
            grok_oauth: result.material.context.clone(),
            vertex_oauth: result.material.vertex_oauth.clone(),
            dim_oauth: result.material.dim_oauth.clone(),
            nous_portal: result.material.nous_portal.clone(),
            lobsterai: result.material.lobsterai.clone(),
            absolute_expiry: result.expires_at.clone(),
            accio_sgcookie: result.material.accio_sgcookie.clone(),
        },
        result.expires_at.clone()?,
    ))
}

impl PendingRefresh {
    pub(super) fn received_unknown(
        owner: std::sync::Arc<TokenRefreshState>,
        attempt: RefreshAttempt,
        original: ProviderCredentialYaml,
        request: GrantRequest,
        record: GrantRecord,
        token: transport::TokenResponse,
    ) -> Self {
        Self {
            owner,
            generation: attempt.generation,
            applied: false,
            provider: request.provider.clone(),
            credential: request.credential.clone(),
            previous_access: attempt.access_token.clone(),
            previous_refresh: attempt.refresh_token.clone(),
            previous_context: attempt.context.clone(),
            previous_sgcookie: attempt.accio_sgcookie.clone(),
            attempt,
            original,
            request,
            record,
            token,
            expires_at: None,
        }
    }

    pub(super) fn received(
        owner: std::sync::Arc<TokenRefreshState>,
        attempt: RefreshAttempt,
        original: ProviderCredentialYaml,
        request: GrantRequest,
        record: GrantRecord,
        token: transport::TokenResponse,
        expires_at: String,
    ) -> Self {
        Self {
            owner,
            generation: attempt.generation,
            applied: false,
            provider: request.provider.clone(),
            credential: request.credential.clone(),
            previous_access: attempt.access_token.clone(),
            previous_refresh: attempt.refresh_token.clone(),
            previous_context: attempt.context.clone(),
            previous_sgcookie: attempt.accio_sgcookie.clone(),
            attempt,
            original,
            request,
            record,
            token,
            expires_at: Some(expires_at),
        }
    }
}
