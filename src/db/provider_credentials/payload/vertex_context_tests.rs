//! Provider defaults cannot manufacture any portion of a Full Vertex user's grant.
use super::*;
use serde_json::json;
#[test]
fn vertex_oauth_context_is_credential_only_and_never_recursively_assembled() {
    let account = json!({"base_url":"https://example.invalid","vertex_oauth":{
        "client_id":"provider-client","client_secret":"provider-private-client",
        "target":{"project":"provider-project","location":"global"}}});
    for credential in [
        json!({}),
        json!({"vertex_oauth":null}),
        json!({"vertex_oauth":{"client_id":"credential-client","target":{"location":"eu"}}}),
    ] {
        let merged = merge_provider_account_and_credential_payloads(&account, &credential);
        assert_eq!(merged.get("vertex_oauth"), credential.get("vertex_oauth"));
        assert!(!merged.to_string().contains("provider-private-client"));
        assert!(!merged.to_string().contains("provider-project"));
        assert_eq!(merged["base_url"], account["base_url"]);
    }
}
