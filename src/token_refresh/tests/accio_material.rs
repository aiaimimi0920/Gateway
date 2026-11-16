//! Synthetic old-format identity and credential ownership; no credentials or upstream access.
use serde_json::json;

#[test]
fn accio_credential_roundtrip_retains_sgcookie() {
    let credential: crate::routing::config::ProviderCredentialYaml =
        serde_json::from_value(json!({"id":"account", "accio_sgcookie":"synthetic-cookie"}))
            .unwrap();
    assert_eq!(
        serde_json::to_value(credential).unwrap()["accio_sgcookie"],
        "synthetic-cookie"
    );
}

#[tokio::test]
#[cfg(feature = "line-accio-web-reverse-api")]
async fn accio_fixed_audience_refresh_is_prepared() {
    let prepared =
        super::accio_guard::prepared(crate::protocol::accio_oauth::REFRESH_ENDPOINT, None).await;
    assert!(
        prepared.is_ok(),
        "complete Accio material must reach its dedicated sender"
    );
}

const LEGACY_RECORD: &str = r#"{"version":1,"attempt":"11111111-1111-4111-8111-111111111111","provider":"oauth","credential":"account","revision":"old-revision","original":{"endpoint":"https://phoenix-gw.alibaba.com/api/auth/refresh_token","client":"synthetic-client","access":"old-access","refresh":"old-refresh","context":null,"expiresAt":"2020-01-01T00:00:00Z"},"phase":"sending","result":null}"#;

#[test]
fn accio_legacy_record_material_and_locator_keep_exact_pre_extension_identities() {
    use crate::console::refresh_grants::{GrantLocator, GrantRecord};
    let record = GrantRecord::from_json(LEGACY_RECORD).unwrap();
    assert_eq!(record.to_json().unwrap(), LEGACY_RECORD);
    assert_eq!(
        record.family_id(),
        "5992d9bbfedabc673e31ccc96e3f3a0827f355d52aef0b2aa63876c1a87611b3"
    );
    assert_eq!(
        record.material_id(),
        "f6c29a449f19067a2a35629ccf9f8170948a9dea4dacaa057fe8889393104230"
    );
    assert_eq!(
        record.identity_digest().unwrap(),
        "b11aaa6357df9f3c77b3e38de84ec77e9047bed78f2d30bd0c2abfbb26169725"
    );
    let locator: GrantLocator = serde_json::from_value(json!({
        "family":"5992d9bbfedabc673e31ccc96e3f3a0827f355d52aef0b2aa63876c1a87611b3",
        "material":"f6c29a449f19067a2a35629ccf9f8170948a9dea4dacaa057fe8889393104230",
        "identity":"b11aaa6357df9f3c77b3e38de84ec77e9047bed78f2d30bd0c2abfbb26169725"
    }))
    .unwrap();
    assert!(locator.matches(&record));
}

#[test]
fn accio_cookie_changes_material_identity_but_never_refresh_family() {
    use crate::console::refresh_grants::GrantRecord;
    let old = GrantRecord::from_json(LEGACY_RECORD).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(LEGACY_RECORD).unwrap();
    value["original"]["accioSgcookie"] = json!("synthetic-cookie");
    let next = GrantRecord::from_json(&value.to_string()).unwrap();
    assert_eq!(old.family_id(), next.family_id());
    assert_ne!(old.material_id(), next.material_id());
    assert_ne!(
        old.identity_digest().unwrap(),
        next.identity_digest().unwrap()
    );
    for cookie in [
        json!(""),
        json!("  "),
        json!("unsafe\n"),
        json!(123),
        json!("x".repeat(16_385)),
    ] {
        value["original"]["accioSgcookie"] = cookie;
        assert!(GrantRecord::from_json(&value.to_string()).is_err());
    }
    value["original"]["accioSgcookie"] = json!("synthetic-cookie");
    value["original"]["endpoint"] = json!("https://foreign.invalid/token");
    assert!(GrantRecord::from_json(&value.to_string()).is_err());
}

#[test]
fn accio_dedicated_fields_cannot_be_forged_by_generic_token_json() {
    let response: super::super::transport::TokenResponse = serde_json::from_value(json!({
        "access_token":"generic-access", "expires_in":60,
        "absolute_expiry":"2099-01-01T00:00:00Z", "accio_sgcookie":"forged-cookie"
    }))
    .unwrap();
    assert!(response.absolute_expiry.is_none());
    assert!(response.accio_sgcookie.is_none());
}

#[test]
fn accio_legacy_without_cookie_does_not_acquire_new_client_or_access_validation() {
    use crate::console::refresh_grants::GrantRecord;
    let mut value: serde_json::Value = serde_json::from_str(LEGACY_RECORD).unwrap();
    value["original"]["client"] = json!("");
    value["original"]["access"] = json!("");
    let legacy = GrantRecord::from_json(&value.to_string()).unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&legacy.to_json().unwrap()).unwrap()["original"]
            .get("accioSgcookie")
            .is_none()
    );
}

#[tokio::test]
#[cfg(not(feature = "line-accio-web-reverse-api"))]
async fn accio_disabled_feature_cannot_prepare_a_grant() {
    let prepared =
        super::accio_guard::prepared(crate::protocol::accio_oauth::REFRESH_ENDPOINT, None).await;
    assert!(prepared.is_err());
    let error = prepared.err().unwrap();
    assert_eq!(error.code, "accio_oauth_feature_disabled");
    assert!(!error.terminal);
}
