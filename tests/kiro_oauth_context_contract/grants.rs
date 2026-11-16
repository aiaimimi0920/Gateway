//! Persisted token-family identity and immutable registered context across Result roundtrips.
use super::*;
use neuro_gateway::console::refresh_grants::GrantRecord;

#[test]
fn kiro_result_roundtrip_keeps_registration_expiry_scopes_and_machine_identity() {
    let original = request(document(context()));
    let result = original
        .result(
            "new-access",
            Some("new-refresh"),
            3600,
            "2099-01-01T00:00:00Z",
            None,
        )
        .unwrap();
    let record = GrantRecord::from_json(&json!({"version":1,"attempt":"11111111-1111-4111-8111-111111111111",
        "provider":"pool","credential":"account","revision":"synthetic-revision",
        "original":{"endpoint":"https://oidc.us-east-1.amazonaws.com/token","client":"synthetic-client",
        "access":"synthetic-access","refresh":"synthetic-refresh","context":null,
        "expiresAt":"2020-01-01T00:00:00Z","kiroOauth":context()},
        "phase":"result","result":result}).to_string()).unwrap();
    let raw = record.to_json().unwrap();
    assert_eq!(
        GrantRecord::from_json(&raw).unwrap().to_json().unwrap(),
        raw
    );
    let value: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(value["original"]["kiroOauth"], context());
    assert_eq!(value["result"]["material"]["kiroOauth"], context());
    for field in ["access", "refresh"] {
        for invalid in [" ".into(), "bad\ntoken".into(), "x".repeat(32_769)] {
            let mut forged = value.clone();
            forged["original"][field] = json!(invalid);
            assert!(
                GrantRecord::from_json(&forged.to_string()).is_err(),
                "accepted invalid original {field}"
            );
        }
    }
    let mut forged = value;
    forged["result"]["material"]["kiroOauth"]["client_secret_expires_at"] = json!(1893456001i64);
    assert!(GrantRecord::from_json(&forged.to_string()).is_err());
}

#[test]
fn kiro_optional_extension_does_not_change_legacy_grant_json_or_material_hash() {
    const LEGACY: &str = r#"{"version":1,"attempt":"11111111-1111-4111-8111-111111111111","provider":"oauth","credential":"account","revision":"old-revision","original":{"endpoint":"https://phoenix-gw.alibaba.com/api/auth/refresh_token","client":"synthetic-client","access":"old-access","refresh":"old-refresh","context":null,"expiresAt":"2020-01-01T00:00:00Z"},"phase":"sending","result":null}"#;
    let record = GrantRecord::from_json(LEGACY).unwrap();
    assert_eq!(record.to_json().unwrap(), LEGACY);
    assert_eq!(
        record.material_id(),
        "f6c29a449f19067a2a35629ccf9f8170948a9dea4dacaa057fe8889393104230"
    );
    assert_eq!(
        record.family_id(),
        "5992d9bbfedabc673e31ccc96e3f3a0827f355d52aef0b2aa63876c1a87611b3"
    );
}
