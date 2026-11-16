//! Credential-only Vertex secrets share normal redaction and complete Keep audience checks.
use super::*;
use serde_json::json;

fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"vertex", "base_url":
    "https://aiplatform.googleapis.com/v1/projects/project-a/locations/global/publishers/google",
    "adapter":"gemini_api_compatible","auth_mode":"vertex_user_oauth",
    "credentials":[{"id":"account","api_key":"synthetic-access",
    "refresh_token":"synthetic-refresh","vertex_oauth":{
        "grant_type":"desktop_authorization_code","client_id":"explicit-client",
        "client_secret":"synthetic-private-client","requested_scope":
        "https://www.googleapis.com/auth/cloud-platform","scope_reported":true,
        "target":{"project":"project-a","location":"global","quota_project":"quota-a"}
    }}]}]}))
    .unwrap()
}
fn path(field: &str) -> String {
    format!("/providers/0/credentials/0/{field}")
}
fn keeps() -> Vec<SecretPatch> {
    ["api_key", "refresh_token", "vertex_oauth/client_secret"]
        .map(|field| SecretPatch::keep(path(field)))
        .into()
}

#[test]
fn vertex_oauth_credential_redaction_preserves_context_and_describes_private_client() {
    let active = document();
    let redacted = redact_route_document(&active).unwrap();
    let value = serde_json::to_value(&redacted.document).unwrap();
    let context = &value["providers"][0]["credentials"][0]["vertex_oauth"];
    assert_eq!(context["client_id"], "explicit-client");
    assert!(context.get("client_secret").is_none());
    assert!(redacted.secrets.iter().any(|descriptor| {
        descriptor.path == path("vertex_oauth/client_secret") && descriptor.configured
    }));
    let raw = value.to_string();
    for secret in [
        "synthetic-access",
        "synthetic-refresh",
        "synthetic-private-client",
    ] {
        assert!(!raw.contains(secret));
    }
    let restored = resolve_secret_patches(&active, redacted.document, &keeps()).unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap(),
        serde_json::to_value(active).unwrap()
    );
}

#[test]
fn vertex_oauth_credential_embedded_private_client_requires_secret_grant_and_patch() {
    let active = document();
    let mut value = serde_json::to_value(redact_route_document(&active).unwrap().document).unwrap();
    value["providers"][0]["credentials"][0]["vertex_oauth"]["client_secret"] = json!("embedded");
    let draft: RouteConfigYaml = serde_json::from_value(value).unwrap();
    assert!(route_config_request_requires_secret_grant(&draft, &[]));
    assert_eq!(
        resolve_secret_patches(&active, draft, &keeps())
            .unwrap_err()
            .code(),
        "secret_value_must_use_patch"
    );
}

#[test]
fn vertex_oauth_credential_keep_binds_client_scope_and_complete_target() {
    let active = document();
    let redacted = redact_route_document(&active).unwrap().document;
    for (pointer, replacement) in [
        ("/client_id", json!("other-client")),
        ("/requested_scope", json!("other-scope")),
        ("/target/project", json!("project-b")),
        ("/target/location", json!("eu")),
        ("/target/quota_project", json!("quota-b")),
    ] {
        let mut value = serde_json::to_value(&redacted).unwrap();
        *value
            .pointer_mut(&format!("/providers/0/credentials/0/vertex_oauth{pointer}"))
            .unwrap() = replacement;
        let draft = serde_json::from_value(value).unwrap();
        let error = resolve_secret_patches(&active, draft, &keeps()).unwrap_err();
        assert_eq!(error.code(), "secret_keep_destination_changed", "{pointer}");
    }
}

#[test]
fn vertex_oauth_credential_model_edit_is_not_a_new_authorization_audience() {
    let active = document();
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].label = Some("renamed".into());
    draft.providers[0].credentials[0].supported_models = vec!["different-model".into()];
    let resolved = resolve_secret_patches(&active, draft, &keeps()).unwrap();
    assert_eq!(
        serde_json::to_value(resolved).unwrap()["providers"][0]["credentials"][0]["vertex_oauth"]
            ["client_secret"],
        "synthetic-private-client"
    );
}

#[test]
fn vertex_oauth_credential_schema_rejects_unknown_context_fields() {
    let mut value = serde_json::to_value(document()).unwrap();
    value["providers"][0]["credentials"][0]["vertex_oauth"]["token_endpoint"] =
        json!("https://evil.invalid");
    assert!(serde_json::from_value::<RouteConfigYaml>(value).is_err());
}
