//! Hosted merge must not inherit or recursively manufacture a credential's registration snapshot.
use super::*;
use serde_json::json;

#[test]
fn kiro_context_is_credential_only_and_never_recursively_assembled() {
    let account = json!({"base_url":"https://q.us-east-1.amazonaws.com", "kiro_oauth":{
        "client_id":"shared-client","client_secret":"shared-secret","machine_id":"shared-machine"}});
    for credential in [
        json!({}),
        json!({"kiro_oauth":null}),
        json!({"kiro_oauth":{"client_id":"credential-client"}}),
    ] {
        let merged = merge_provider_account_and_credential_payloads(&account, &credential);
        assert_eq!(merged.get("kiro_oauth"), credential.get("kiro_oauth"));
        assert_eq!(merged["base_url"], account["base_url"]);
        assert!(!merged.to_string().contains("shared-secret"));
    }
}
