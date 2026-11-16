//! Existing SecretPatch machinery owns the secret; context changes require replacement consent.
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use serde_json::json;
#[path = "fixtures/luma_mcp.rs"]
mod fixture;

#[test]
fn luma_mcp_redaction_and_keep_preserve_context_and_secret_separately() {
    let active = fixture::document(fixture::provider());
    let redacted = redact_route_document(&active).unwrap();
    let raw = serde_json::to_value(&redacted.document).unwrap();
    assert!(!raw.to_string().contains("synthetic-access"));
    assert_eq!(
        raw["providers"][0]["credentials"][0]["luma_oauth"]["client_id"],
        "registered-client"
    );
    let saved = resolve_secret_patches(
        &active,
        redacted.document,
        &[SecretPatch::keep("/providers/0/credentials/0/api_key")],
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(saved).unwrap(),
        serde_json::to_value(active).unwrap()
    );
}

#[test]
fn luma_mcp_keep_cannot_rebind_client_redirect_expiry_or_product() {
    let mut p = fixture::provider();
    p["credentials"][0]["luma_oauth"]["expires_at"] = json!("2030-01-01T00:00:00Z");
    let active = fixture::document(p);
    let redacted = redact_route_document(&active).unwrap().document;
    for (pointer, value) in [
        ("/adapter", json!("openai_compatible")),
        ("/base_url", json!("https://agents.lumalabs.ai/v1")),
        ("/credentials/0/luma_oauth/client_id", json!("other-client")),
        (
            "/credentials/0/luma_oauth/redirect_uri",
            json!("http://127.0.0.1:54422/callback"),
        ),
        (
            "/credentials/0/luma_oauth/expires_at",
            json!("2040-01-01T00:00:00Z"),
        ),
        (
            "/credentials/0/luma_oauth/expires_at",
            serde_json::Value::Null,
        ),
        ("/credentials/0/luma_oauth", serde_json::Value::Null),
    ] {
        let mut raw = serde_json::to_value(&redacted).unwrap();
        *raw.pointer_mut(&format!("/providers/0{pointer}")).unwrap() = value;
        let error = resolve_secret_patches(
            &active,
            serde_json::from_value(raw).unwrap(),
            &[SecretPatch::keep("/providers/0/credentials/0/api_key")],
        )
        .unwrap_err();
        assert_eq!(error.code(), "secret_keep_destination_changed", "{pointer}");
    }
}

#[test]
fn luma_mcp_context_is_never_inherited_from_a_database_parent() {
    use neuro_gateway::db::merge_provider_account_and_credential_payloads;
    let context = fixture::provider()["credentials"][0]["luma_oauth"].clone();
    let account = json!({"luma_oauth":context,"adapter":"lumalabs_mcp_readonly"});
    let merged =
        merge_provider_account_and_credential_payloads(&account, &json!({"api_key":"synthetic"}));
    assert!(merged.get("luma_oauth").is_none());
    let merged =
        merge_provider_account_and_credential_payloads(&account, &json!({"luma_oauth":context}));
    assert_eq!(merged["luma_oauth"], context);
}
