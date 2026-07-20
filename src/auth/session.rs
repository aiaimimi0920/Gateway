// ---------------------------------------------------------------------------
// AuthenticatedSession — the principal attached to a successfully authed request
// ---------------------------------------------------------------------------

/// Represents a verified caller identity after the auth layer has accepted the
/// request.  All fields are set by the [`AuthAdapter`] that processed the
/// request and should be treated as authoritative by the rest of the pipeline.
///
/// [`AuthAdapter`]: super::adapter::AuthAdapter
#[derive(Debug, Clone)]
pub struct AuthenticatedSession {
    /// Platform project that owns this request.
    pub project_id: String,

    /// Tenant (organisation) associated with the project.
    pub tenant_id: String,

    /// Optional end-user identifier, present when the caller supplies a `user`
    /// field in the request body.
    pub user_id: Option<String>,

    /// Opaque reference to the credential record used to authenticate this
    /// request (e.g. a database row ID), useful for audit logs.
    pub credential_ref: Option<String>,

    /// Permission scopes granted to this session (e.g. `["inference:chat"]`).
    pub scopes: Vec<String>,

    /// The ID of the API key that was used, if authentication was key-based.
    pub api_key_id: Option<String>,

    /// The ID of the user credential that was used, if authentication was
    /// performed with a `gw-user-*` credential.
    pub user_credential_id: Option<String>,

    /// Unified access key ID backing this session.
    pub access_key_id: Option<String>,

    /// Unified access key kind (`normal` or `auto_route`).
    pub access_key_kind: Option<String>,
}

impl AuthenticatedSession {
    /// Returns `true` if the session has the requested scope.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(scopes: Vec<&str>) -> AuthenticatedSession {
        AuthenticatedSession {
            project_id: "proj-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            user_id: None,
            credential_ref: None,
            scopes: scopes.into_iter().map(str::to_string).collect(),
            api_key_id: Some("key-abc".to_string()),
            user_credential_id: None,
            access_key_id: Some("access-key-1".to_string()),
            access_key_kind: Some("normal".to_string()),
        }
    }

    #[test]
    fn has_scope_returns_true_for_granted_scope() {
        let s = make_session(vec!["inference:chat", "inference:embed"]);
        assert!(s.has_scope("inference:chat"));
        assert!(s.has_scope("inference:embed"));
    }

    #[test]
    fn has_scope_returns_false_for_missing_scope() {
        let s = make_session(vec!["inference:chat"]);
        assert!(!s.has_scope("admin:write"));
    }

    #[test]
    fn has_scope_returns_false_for_empty_scopes() {
        let s = make_session(vec![]);
        assert!(!s.has_scope("inference:chat"));
    }

    #[test]
    fn session_fields_are_accessible() {
        let s = make_session(vec!["inference:chat"]);
        assert_eq!(s.project_id, "proj-1");
        assert_eq!(s.tenant_id, "tenant-1");
        assert_eq!(s.api_key_id.as_deref(), Some("key-abc"));
        assert!(s.user_credential_id.is_none());
        assert_eq!(s.access_key_id.as_deref(), Some("access-key-1"));
        assert_eq!(s.access_key_kind.as_deref(), Some("normal"));
        assert!(s.user_id.is_none());
    }
}
