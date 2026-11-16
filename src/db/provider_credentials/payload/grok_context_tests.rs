//! A credential authorization cannot be inherited or recursively built from account defaults.
use super::*;
use serde_json::json;

#[test]
fn grok_context_is_credential_only_and_replaced_as_a_whole() {
    let account = json!({"base_url":"https://cli-chat-proxy.grok.com/v1",
        "grok_oauth":{"issuer":"https://auth.x.ai","client_id":"account-client","principal":{"id":"account-principal"}}});
    for credential in [
        json!({}),
        json!({"grok_oauth":null}),
        json!({"grok_oauth":{"client_id":"credential-client"}}),
    ] {
        let merged = merge_provider_account_and_credential_payloads(&account, &credential);
        assert_eq!(merged.get("grok_oauth"), credential.get("grok_oauth"));
        assert_eq!(merged["base_url"], account["base_url"]);
    }
}
