// ---------------------------------------------------------------------------
// Auth adapter trait + implementations
//
// The `AuthAdapter` trait abstracts how an incoming HTTP request is
// authenticated.  Concrete implementations are swapped in through
// configuration, allowing dev/test environments to use lightweight static
// keys while production uses a full token-validation backend.
// ---------------------------------------------------------------------------

use std::collections::HashMap;

use async_trait::async_trait;

use super::session::AuthenticatedSession;
use crate::error::GatewayError;

// ---------------------------------------------------------------------------
// AuthRequest
// ---------------------------------------------------------------------------

/// The auth-relevant fields extracted from an incoming HTTP request before the
/// body has been parsed.
#[derive(Debug, Clone)]
pub struct AuthRequest {
    /// Value of the `Authorization` header, if present.
    pub authorization: Option<String>,

    /// Value of the `x-api-key` header, if present.
    pub api_key: Option<String>,

    /// Request path (e.g. `/v1/chat/completions`).
    pub path: String,

    /// HTTP method (e.g. `"POST"`).
    pub method: String,
}

impl AuthRequest {
    /// Extract the bearer token from the `Authorization` header.
    ///
    /// Returns `None` if the header is absent or doesn't start with `"Bearer "`.
    pub fn bearer_token(&self) -> Option<&str> {
        self.authorization
            .as_deref()
            .and_then(|v| v.strip_prefix("Bearer "))
    }

    /// Return the best available credential string: `api_key` header first,
    /// then bearer token from `Authorization`.
    pub fn credential(&self) -> Option<&str> {
        self.api_key.as_deref().or_else(|| self.bearer_token())
    }
}

// ---------------------------------------------------------------------------
// AuthResult
// ---------------------------------------------------------------------------

/// The outcome of calling [`AuthAdapter::authenticate`].
#[derive(Debug)]
pub enum AuthResult {
    /// The request is authenticated.  The gateway proceeds with the attached
    /// session.
    Authenticated(AuthenticatedSession),

    /// The request was rejected by the auth adapter.
    Rejected {
        /// Human-readable error message to return to the caller.
        error: String,
        /// Suggested HTTP status code (typically 401 or 403).
        status_code: u16,
    },
}

impl AuthResult {
    /// Convenience: unwrap to `AuthenticatedSession` or convert the rejection
    /// into a [`GatewayError`].
    pub fn into_session(self) -> Result<AuthenticatedSession, GatewayError> {
        match self {
            AuthResult::Authenticated(session) => Ok(session),
            AuthResult::Rejected { error, .. } => Err(GatewayError::unauthorized(error)),
        }
    }
}

// ---------------------------------------------------------------------------
// AuthAdapter trait
// ---------------------------------------------------------------------------

/// An authentication adapter.  Implement this trait to add a new auth backend.
#[async_trait]
pub trait AuthAdapter: Send + Sync {
    /// Short identifying name for this adapter (used in logs and metrics).
    fn name(&self) -> &str;

    /// Attempt to authenticate the incoming request.
    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthResult, GatewayError>;
}

// ---------------------------------------------------------------------------
// StaticKeyEntry
// ---------------------------------------------------------------------------

/// Metadata associated with a single static API key.
#[derive(Debug, Clone)]
pub struct StaticKeyEntry {
    /// Project the key belongs to.
    pub project_id: String,

    /// Tenant the key belongs to.
    pub tenant_id: String,

    /// Permission scopes granted to this key.
    pub scopes: Vec<String>,
}

// ---------------------------------------------------------------------------
// StaticKeyAdapter
// ---------------------------------------------------------------------------

/// A simple, in-memory adapter that validates requests against a fixed map of
/// API keys.  Intended for **development and testing only** — do not use in
/// production environments where keys are sensitive.
///
/// Accepted credential sources (checked in order):
/// 1. `x-api-key` request header
/// 2. `Authorization: Bearer <key>` header
pub struct StaticKeyAdapter {
    /// Map from raw key string → entry.
    pub keys: HashMap<String, StaticKeyEntry>,
}

impl StaticKeyAdapter {
    /// Create an adapter from an iterator of `(key, entry)` pairs.
    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, StaticKeyEntry)>) -> Self {
        Self {
            keys: pairs.into_iter().collect(),
        }
    }
}

#[async_trait]
impl AuthAdapter for StaticKeyAdapter {
    fn name(&self) -> &str {
        "static_key"
    }

    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthResult, GatewayError> {
        let credential = match request.credential() {
            Some(c) => c,
            None => {
                return Ok(AuthResult::Rejected {
                    error: "Missing API key or Authorization header".to_string(),
                    status_code: 401,
                });
            }
        };

        match self.keys.get(credential) {
            Some(entry) => Ok(AuthResult::Authenticated(AuthenticatedSession {
                project_id: entry.project_id.clone(),
                tenant_id: entry.tenant_id.clone(),
                user_id: None,
                credential_ref: None,
                scopes: entry.scopes.clone(),
                api_key_id: Some(credential.to_string()),
                user_credential_id: None,
                access_key_id: None,
                access_key_kind: None,
            })),
            None => Ok(AuthResult::Rejected {
                error: "Invalid API key".to_string(),
                status_code: 401,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_adapter() -> StaticKeyAdapter {
        StaticKeyAdapter::from_pairs([
            (
                "valid-key-1".to_string(),
                StaticKeyEntry {
                    project_id: "proj-a".to_string(),
                    tenant_id: "tenant-a".to_string(),
                    scopes: vec!["inference:chat".to_string()],
                },
            ),
            (
                "admin-key".to_string(),
                StaticKeyEntry {
                    project_id: "proj-b".to_string(),
                    tenant_id: "tenant-b".to_string(),
                    scopes: vec!["inference:chat".to_string(), "admin:read".to_string()],
                },
            ),
        ])
    }

    fn make_request(api_key: Option<&str>, authorization: Option<&str>) -> AuthRequest {
        AuthRequest {
            authorization: authorization.map(str::to_string),
            api_key: api_key.map(str::to_string),
            path: "/v1/chat/completions".to_string(),
            method: "POST".to_string(),
        }
    }

    // ── AuthRequest helpers ───────────────────────────────────────────────

    #[test]
    fn bearer_token_extracts_from_authorization_header() {
        let req = make_request(None, Some("Bearer my-secret-token"));
        assert_eq!(req.bearer_token(), Some("my-secret-token"));
    }

    #[test]
    fn bearer_token_returns_none_when_no_header() {
        let req = make_request(None, None);
        assert_eq!(req.bearer_token(), None);
    }

    #[test]
    fn bearer_token_returns_none_for_non_bearer_scheme() {
        let req = make_request(None, Some("Basic dXNlcjpwYXNz"));
        assert_eq!(req.bearer_token(), None);
    }

    #[test]
    fn credential_prefers_api_key_over_bearer() {
        let req = make_request(Some("from-header"), Some("Bearer from-auth"));
        assert_eq!(req.credential(), Some("from-header"));
    }

    #[test]
    fn credential_falls_back_to_bearer() {
        let req = make_request(None, Some("Bearer fallback-key"));
        assert_eq!(req.credential(), Some("fallback-key"));
    }

    #[test]
    fn credential_returns_none_when_no_auth_provided() {
        let req = make_request(None, None);
        assert_eq!(req.credential(), None);
    }

    // ── StaticKeyAdapter ──────────────────────────────────────────────────

    #[tokio::test]
    async fn valid_api_key_authenticates() {
        let adapter = make_adapter();
        let req = make_request(Some("valid-key-1"), None);
        let result = adapter.authenticate(&req).await.unwrap();
        match result {
            AuthResult::Authenticated(session) => {
                assert_eq!(session.project_id, "proj-a");
                assert_eq!(session.tenant_id, "tenant-a");
                assert!(session.has_scope("inference:chat"));
                assert_eq!(session.api_key_id.as_deref(), Some("valid-key-1"));
            }
            AuthResult::Rejected { error, .. } => {
                panic!("expected Authenticated, got Rejected: {}", error)
            }
        }
    }

    #[tokio::test]
    async fn valid_bearer_token_authenticates() {
        let adapter = make_adapter();
        let req = make_request(None, Some("Bearer admin-key"));
        let result = adapter.authenticate(&req).await.unwrap();
        match result {
            AuthResult::Authenticated(session) => {
                assert_eq!(session.project_id, "proj-b");
                assert!(session.has_scope("admin:read"));
            }
            AuthResult::Rejected { error, .. } => panic!("expected Authenticated, got: {}", error),
        }
    }

    #[tokio::test]
    async fn unknown_key_is_rejected() {
        let adapter = make_adapter();
        let req = make_request(Some("bad-key"), None);
        let result = adapter.authenticate(&req).await.unwrap();
        match result {
            AuthResult::Rejected { status_code, .. } => assert_eq!(status_code, 401),
            AuthResult::Authenticated(_) => panic!("expected Rejected"),
        }
    }

    #[tokio::test]
    async fn missing_credentials_are_rejected() {
        let adapter = make_adapter();
        let req = make_request(None, None);
        let result = adapter.authenticate(&req).await.unwrap();
        match result {
            AuthResult::Rejected { status_code, .. } => assert_eq!(status_code, 401),
            AuthResult::Authenticated(_) => panic!("expected Rejected"),
        }
    }

    #[tokio::test]
    async fn rejected_result_converts_to_gateway_error() {
        let adapter = make_adapter();
        let req = make_request(Some("nope"), None);
        let result = adapter.authenticate(&req).await.unwrap();
        let err = result.into_session().unwrap_err();
        assert!(matches!(err.kind, crate::error::ErrorKind::Authentication));
    }

    #[test]
    fn adapter_name_is_static_key() {
        let adapter = make_adapter();
        assert_eq!(adapter.name(), "static_key");
    }
}
