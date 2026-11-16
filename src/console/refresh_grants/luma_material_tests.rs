//! Offline consent and exact authority are part of the durable identity, not mutable policy.
use super::{luma_tests::revision, *};
use serde_json::json;
use sha2::{Digest, Sha256};

fn request() -> GrantRequest {
    GrantRequest::from_revision(
        &revision(Some("2020-01-01T00:00:00Z")),
        "luma-mcp",
        "saved-mcp",
    )
    .unwrap()
}

#[test]
fn luma_grant_material_rejects_authority_confusion_and_malformed_secrets() {
    let request = request();
    let raw = serde_json::to_value(&request.material).unwrap();
    // The protocol's opaque refresh grammar includes ASCII space; do not apply generic trimming.
    let mut opaque = raw.clone();
    opaque["refresh"] = json!(" ");
    let opaque: GrantMaterial = serde_json::from_value(opaque).unwrap();
    opaque.validate().unwrap();
    for (pointer, value) in [
        ("/endpoint", json!("https://other.invalid/token")),
        ("/client", json!("another-client")),
        ("/access", json!("bad\naccess")),
        ("/refresh", json!("")),
        ("/refresh", json!("bad\nrefresh")),
        ("/lumaOauth", serde_json::Value::Null),
        ("/lumaOauth/offline_access", json!(false)),
        ("/lumaOauth/expires_at", json!("2030-01-01T00:00:00Z")),
        ("/expiresAt", serde_json::Value::Null),
    ] {
        let mut changed = raw.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        let changed: GrantMaterial = serde_json::from_value(changed).unwrap();
        assert!(changed.validate().is_err(), "{pointer}");
    }
    let mut changed = raw;
    changed["accioSgcookie"] = json!("synthetic-cookie");
    let changed: GrantMaterial = serde_json::from_value(changed).unwrap();
    assert!(changed.validate().is_err());
}

#[test]
fn luma_grant_context_is_bound_without_changing_legacy_identity() {
    let request = request();
    let mut raw = serde_json::to_value(&request.material).unwrap();
    raw["lumaOauth"]["redirect_uri"] = json!("http://127.0.0.1:54422/callback");
    let changed: GrantMaterial = serde_json::from_value(raw).unwrap();
    changed.validate().unwrap();
    assert_eq!(request.family_id(), changed.family_id());
    assert_ne!(request.material_id(), changed.material_id());
    assert!(!request.material.equivalent(&changed));
    let old: GrantMaterial =
        serde_json::from_value(json!({"endpoint":"https://issuer.invalid/token",
        "client":"client","access":"access","refresh":"refresh","context":null,"expiresAt":null}))
        .unwrap();
    old.validate().unwrap();
    assert!(!old.permits(&request.material));
    assert!(!request.material.permits(&old));
    assert!(serde_json::to_value(&old)
        .unwrap()
        .get("lumaOauth")
        .is_none());
    let legacy_id = json!([
        "gateway-refresh-material-v1",
        old.endpoint,
        old.client,
        old.refresh,
        old.access,
        null,
        null
    ]);
    assert_eq!(
        old.material_id(),
        hex::encode(Sha256::digest(legacy_id.to_string().as_bytes()))
    );
    let record = GrantRecord::sending(&request);
    let restored = GrantRecord::from_json(&record.to_json().unwrap()).unwrap();
    assert_eq!(restored, record);
    assert!(!format!("{restored:?}").contains("old-refresh"));
}

#[test]
fn luma_grant_result_requires_receipt_and_rejects_generic_lifetime() {
    let request = request();
    let mut stripped = request.material.clone();
    stripped.luma_oauth = None;
    assert!(stripped.validate().is_err());
    assert!(!stripped.permits(&request.material));
    assert!(request.material.permits(&request.material));
    assert!(request
        .result(
            "new-access",
            Some("new-refresh"),
            3600,
            "2020-01-01T00:00:00Z",
            None
        )
        .is_err());
    let record = GrantRecord::sending(&request);
    let mut raw: serde_json::Value = serde_json::from_str(&record.to_json().unwrap()).unwrap();
    raw["phase"] = json!("result");
    raw["result"] =
        json!({"material":request.material,"expiresIn":3600,"expiresAt":"2020-01-01T00:00:00Z"});
    assert!(GrantRecord::from_json(&raw.to_string()).is_err());
    raw["phase"] = json!("published");
    assert!(GrantRecord::from_json(&raw.to_string()).is_err());
    assert!(record.replay_for(&request).is_none());
}
