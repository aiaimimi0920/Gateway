//! Synthetic offline accounts exercise the existing permanent grant owner, never a real token endpoint.
use super::*;
use crate::console::document::validate_route_document;
use serde_json::json;

pub(super) fn revision(expiry: Option<&str>) -> RouteConfigRedisRevision {
    let mut context = json!({"issuer":"https://agents.lumalabs.ai",
        "resource":"https://agents.lumalabs.ai/mcp","token_endpoint":"https://auth.lumalabs.ai/oauth2/token",
        "client_id":"registered-client","redirect_uri":"http://127.0.0.1:54421/callback","offline_access":true});
    if let Some(expiry) = expiry {
        context["expires_at"] = json!(expiry);
    }
    let document = serde_json::from_value(json!({"providers":[{"id":"luma-mcp",
        "base_url":"https://agents.lumalabs.ai/mcp","adapter":"lumalabs_mcp_readonly",
        "protocol_profile":"lumalabs_mcp_readonly","protocol_family":"lumalabs_mcp",
        "auth_mode":"luma_mcp_oauth","explicit_credentials_only":true,
        "credentials":[{"id":"saved-mcp","enabled":false,"api_key":"old-access",
            "refresh_token":"old-refresh","luma_oauth":context}]}]}))
    .unwrap();
    let validated = validate_route_document(document).unwrap();
    let metadata = crate::console::revision::RevisionMetadata::from_validated(
        0,
        None,
        crate::console::revision::RevisionActor::Bootstrap,
        time::OffsetDateTime::now_utc(),
        None,
        &validated,
    );
    RouteConfigRedisRevision::new(metadata, validated.document().clone()).unwrap()
}

#[test]
fn luma_grant_request_derives_fixed_authority_from_offline_context() {
    for expiry in [None, Some("2020-01-01T00:00:00Z")] {
        let revision = revision(expiry);
        let request = GrantRequest::from_revision(&revision, "luma-mcp", "saved-mcp");
        assert!(
            request.is_ok(),
            "offline Luma must enter durable grant admission: {request:?}"
        );
        let request = request.unwrap();
        assert_eq!(
            request.material.endpoint,
            crate::protocol::luma_oauth::TOKEN_ENDPOINT
        );
        assert_eq!(request.material.client, "registered-client");
        assert_eq!(request.material.expires_at.as_deref(), expiry);
        assert!(!format!("{request:?}").contains("old-refresh"));
    }
}
