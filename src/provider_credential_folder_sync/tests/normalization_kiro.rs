//! The real folder dispatch must not retain ambiguous auth copies or rotate the old device identity.
use super::super::normalization::normalize_import_payload;
use super::build_test_provider_account;
use serde_json::{json, Value};
mod safety;

fn provider() -> crate::db::GatewayProviderAccountView {
    let mut provider = build_test_provider_account(
        "Kiro",
        "kiro_compatible",
        "kiro",
        "kiro",
        "https://q.us-east-1.amazonaws.com",
        None,
        None,
    );
    provider.service_provider_key = "kiro_platform".into();
    provider.service_provider_label = "Kiro".into();
    provider
}

fn legacy() -> Value {
    json!({"apiKey":"synthetic-access", "refreshToken":"synthetic-refresh",
        "extraBody":{"kiroRefreshToken":"synthetic-refresh",
            "kiroAuthMethod":"builder_id", "kiroAuthRegion":"us-east-1",
            "startUrl":"https://view.awsapps.com/start", "kiroClientId":"synthetic-client",
            "kiroClientSecret":"synthetic-registered-secret", "clientSecretExpiresAt":1893456000i64,
            "scopes":["codewhisperer:conversations"], "kiroVersion":"0.7.1"}})
}

fn normalize(raw: Value) -> Result<Value, crate::error::GatewayError> {
    normalize_import_payload("kiro", &provider(), raw, None)
}

#[test]
fn kiro_folder_dispatch_promotes_complete_registration_without_auth_duplicates() {
    let result = normalize(legacy()).unwrap();
    assert_eq!(result["api_key"], "synthetic-access");
    assert_eq!(result["refresh_token"], "synthetic-refresh");
    assert_eq!(
        result["kiro_oauth"]["client_secret"],
        "synthetic-registered-secret"
    );
    assert_eq!(result["session_auth"]["transport"], "bearer");
    assert_eq!(result["extra_body"]["kiroVersion"], "0.7.1");
    assert!(result.get("apiKey").is_none());
    assert!(result.get("rawSource").is_none());
    assert!(result["extra_body"]
        .as_object()
        .unwrap()
        .keys()
        .all(|name| !crate::protocol::kiro_oauth::business::auth_extra(name)));
}

#[test]
fn kiro_folder_dispatch_rejects_conflicting_standard_and_legacy_refresh() {
    let mut raw = legacy();
    raw["refresh_token"] = json!("a-different-private-refresh");
    let error = normalize(raw).expect_err("aliases must not be selected by map order");
    assert!(!error.message.contains("a-different-private-refresh"));
    assert!(!error.message.contains("synthetic-registered-secret"));
}

#[test]
fn kiro_folder_dispatch_freezes_the_preexisting_derived_machine() {
    let expected =
        crate::protocol::kiro::generate_machine_id(None, Some("synthetic-refresh")).unwrap();
    let result = normalize(legacy()).unwrap();
    assert_eq!(result["kiro_oauth"]["machine_id"], expected);
    assert_eq!(result["extra_body"]["kiroMachineId"], expected);
}
