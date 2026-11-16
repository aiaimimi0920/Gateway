//! A received response is preserved before interpretation; restored receipts cannot renew expiry.
use super::*;
use crate::protocol::luma_oauth::{transport_fixture::*, ReceivedRefresh, ISSUER, RESOURCE};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::json;

fn context() -> AuthorizationContext {
    serde_json::from_value(
        json!({"issuer":ISSUER,"resource":RESOURCE,"token_endpoint":TOKEN_ENDPOINT,
        "client_id":"receipt-client","redirect_uri":"http://127.0.0.1:34567/callback",
        "offline_access":true,"expires_at":"2020-01-01T00:00:00Z"}),
    )
    .unwrap()
}
fn capture(raw: &[u8], at: OffsetDateTime) -> ReceivedRefresh {
    ReceivedRefresh::capture(raw, at, &context(), "old-refresh").unwrap()
}

#[test]
fn luma_received_roundtrip_keeps_malformed_and_duplicate_responses_without_admission() {
    for raw in [b"not-json".as_slice(), &[255, 0, 254],
        br#"{"access_token":"new","token_type":"Bearer","expires_in":false,"refresh_token":"successor"}"#,
        br#"{"access_token":"new","token_type":"Bearer","refresh_token":"a","refresh_token":"b"}"#,
        br#"{"access_token":"new","token_type":"Bearer","refresh_token":null}"#,
        br#"{"access_token":"new","access_token":"other","token_type":"Bearer"}"#,
        br#"{"access_token":"new","token_type":"Bearer","expires_in":0}"#,
        br#"{"access_token":"new","token_type":"Bearer","expires_in":-1}"#,
        br#"{"access_token":"new","token_type":"Bearer","expires_in":18446744073709551615}"#,
        br#"{"access_token":"new","token_type":"Bearer","expires_in":1,"expires_in":2}"#,
        br#"{"access_token":"new","token_type":"Bearer","error":"private-error"}"#] {
        let received = capture(raw, OffsetDateTime::now_utc());
        let encoded = serde_json::to_value(&received).unwrap();
        assert_eq!(STANDARD.decode(encoded["body"].as_str().unwrap()).unwrap(), raw);
        let restored: ReceivedRefresh = serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(received, restored);
        restored.validate_storage().unwrap();
        assert!(restored.interpret_successor(&context(), "old-refresh").is_err());
        assert!(!format!("{restored:?}").contains(encoded["body"].as_str().unwrap()));
    }
}

#[test]
fn luma_received_uses_send_time_and_preserves_unknown_expiry_and_rotation_rules() {
    let at = OffsetDateTime::now_utc() - time::Duration::seconds(60);
    let raw = br#"{"access_token":"new","token_type":"Bearer","expires_in":3600,"refresh_token":"successor"}"#;
    let received = capture(raw, at);
    let (material, refresh) = received
        .interpret_successor(&context(), "old-refresh")
        .unwrap()
        .into_parts();
    assert_eq!(
        material.expires_at(),
        Some(at + time::Duration::seconds(3600))
    );
    assert!(material.offline_access());
    assert_eq!(refresh, "successor");
    let expired = capture(raw, at - time::Duration::hours(2));
    expired.validate_storage().unwrap();
    assert_eq!(
        expired
            .interpret_successor(&context(), "old-refresh")
            .unwrap_err(),
        OAuthError::Expired
    );
    let unknown = capture(br#"{"access_token":"new","token_type":"Bearer"}"#, at);
    let (material, refresh) = unknown
        .interpret_successor(&context(), "old-refresh")
        .unwrap()
        .into_parts();
    assert!(material.expires_at().is_none());
    assert_eq!(refresh, "old-refresh");
}

#[test]
fn luma_received_storage_bounds_and_grant_binding_are_rechecked_after_deserialization() {
    let received = capture(b"{}", OffsetDateTime::now_utc());
    let raw = serde_json::to_value(&received).unwrap();
    for (field, value) in [
        ("body", json!("%")),
        ("body", json!("A".repeat(87388))),
        ("body", json!("Zh==")),
        ("body", json!("Zg")),
        ("body", json!("Zg==\n")),
        ("body", json!(STANDARD.encode(vec![0; 65537]))),
        ("binding", json!("0")),
        ("binding", json!("G".repeat(64))),
    ] {
        let mut invalid = raw.clone();
        invalid[field] = value;
        let invalid: ReceivedRefresh = serde_json::from_value(invalid).unwrap();
        assert!(invalid.validate_storage().is_err());
    }
    let mut invalid = raw.clone();
    invalid["startedAt"] = json!("not-a-time");
    assert!(serde_json::from_value::<ReceivedRefresh>(invalid).is_err());
    let mut invalid = raw;
    invalid["unexpected"] = json!(true);
    assert!(serde_json::from_value::<ReceivedRefresh>(invalid).is_err());
    for (field, value) in [
        ("client_id", json!("other")),
        ("redirect_uri", json!("http://127.0.0.1:34568/callback")),
        ("offline_access", json!(false)),
        ("expires_at", json!("2021-01-01T00:00:00Z")),
    ] {
        let mut changed = serde_json::to_value(context()).unwrap();
        changed[field] = value;
        let changed: AuthorizationContext = serde_json::from_value(changed).unwrap();
        assert!(received.validate_binding(&changed, "old-refresh").is_err());
    }
    assert!(received
        .validate_binding(&context(), "other-refresh")
        .is_err());
    capture(&vec![255; 65536], OffsetDateTime::now_utc())
        .validate_storage()
        .unwrap();
    assert!(matches!(
        ReceivedRefresh::capture(
            &vec![0; 65537],
            OffsetDateTime::now_utc(),
            &context(),
            "old-refresh"
        ),
        Err(OAuthError::ResponseTooLarge)
    ));
}

#[tokio::test]
async fn luma_received_transport_captures_before_admission_and_never_replays() {
    let raw = r#"{"access_token":"new","token_type":"Bearer","expires_in":false,"refresh_token":"received-successor"}"#;
    let mock = Mock::new(Reply {
        body: raw.into(),
        ..Default::default()
    })
    .await;
    let received = crate::protocol::luma_oauth::test_support::ENDPOINT
        .scope(
            mock.endpoint.clone(),
            RefreshGrant::new(context(), "old-refresh".into())
                .unwrap()
                .receive(&http()),
        )
        .await
        .unwrap();
    assert_eq!(mock.count(), 1);
    let stored = serde_json::to_string(&received).unwrap();
    let restored: ReceivedRefresh = serde_json::from_str(&stored).unwrap();
    for _ in 0..2 {
        assert!(restored
            .interpret_successor(&context(), "old-refresh")
            .is_err());
    }
    assert_eq!(mock.count(), 1);
    assert_eq!(
        STANDARD
            .decode(
                serde_json::to_value(restored).unwrap()["body"]
                    .as_str()
                    .unwrap()
            )
            .unwrap(),
        raw.as_bytes()
    );
    mock.stop().await;
}
