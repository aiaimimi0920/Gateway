use super::*;
use serde_json::json;
#[test]
fn freebuff_session_receipt_is_credential_only_and_never_recursively_assembled() {
    let account = json!({"base_url":"https://freebuff.com","freebuff_session":{
        "fingerprint_id":"provider-device","fingerprint_hash":"provider-private-hash"}});
    for credential in [
        json!({}),
        json!({"freebuff_session":null}),
        json!({"freebuff_session":{"fingerprint_id":"credential-device"}}),
    ] {
        let merged = merge_provider_account_and_credential_payloads(&account, &credential);
        assert_eq!(
            merged.get("freebuff_session"),
            credential.get("freebuff_session")
        );
        assert!(!merged.to_string().contains("provider-private-hash"));
        assert!(!merged.to_string().contains("provider-device"));
        assert_eq!(merged["base_url"], account["base_url"]);
    }
}
