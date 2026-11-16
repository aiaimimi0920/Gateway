//! A parent payload cannot manufacture or complete a DIM subject/client authorization.
use super::*;
use serde_json::json;
#[test]
fn dim_context_is_credential_owned_and_not_recursively_merged() {
    let account = json!({"base_url":"https://dimagent.cn/v1","dim_oauth":{
        "client_id":"parent-client","subject":"parent-subject","nonce":"parent-nonce"}});
    for credential in [
        json!({}),
        json!({"dim_oauth":null}),
        json!({"dim_oauth":{"client_id":"credential-client"}}),
    ] {
        let merged = merge_provider_account_and_credential_payloads(&account, &credential);
        assert_eq!(merged.get("dim_oauth"), credential.get("dim_oauth"));
        assert!(!merged.to_string().contains("parent-subject"));
        assert_eq!(merged["base_url"], account["base_url"]);
    }
}
