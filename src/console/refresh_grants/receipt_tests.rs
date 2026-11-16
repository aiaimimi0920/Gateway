//! Receipt admission never changes the original grant identity or widens exact-revision recovery.
use super::*;
use crate::protocol::nous_portal::{ReceivedTokens, TOKEN_ENDPOINT};
use serde_json::json;

fn request() -> GrantRequest {
    GrantRequest {
        provider: "provider".into(),
        credential: "credential".into(),
        revision: "revision".into(),
        material: serde_json::from_value(
            json!({"endpoint":TOKEN_ENDPOINT,"client":"permitted-client",
            "access":"old-access","refresh":"old-refresh","context":null,"expiresAt":null}),
        )
        .unwrap(),
    }
}
fn receipt() -> GrantReceipt {
    GrantReceipt::NousPortal { tokens: ReceivedTokens::from_json(
        br#"{"access_token":"unusable-jwt","refresh_token":"received-successor","scope":false}"#).unwrap() }
}

#[test]
fn nous_receipt_keeps_the_permanent_locator_and_exact_authorization_binding() {
    let request = request();
    let sending = GrantRecord::sending(&request);
    let saved = sending.with_receipt(&receipt()).unwrap();
    assert_eq!(
        sending.identity_digest().unwrap(),
        saved.identity_digest().unwrap()
    );
    let locator = GrantLocator::from_record(&sending).unwrap();
    assert!(locator.matches(&saved));
    assert!(saved.replay_for(&request).is_none());
    assert!(saved.receipt_for(&request).is_some());
    for changed in [
        "provider",
        "credential",
        "revision",
        "access",
        "refresh",
        "client",
    ] {
        let mut other = request.clone();
        match changed {
            "provider" => other.provider.push('x'),
            "credential" => other.credential.push('x'),
            "revision" => other.revision.push('x'),
            "access" => other.material.access.push('x'),
            "refresh" => other.material.refresh.push('x'),
            "client" => other.material.client.push('x'),
            _ => unreachable!(),
        }
        assert!(saved.receipt_for(&other).is_none(), "{changed}");
    }
}

#[test]
fn nous_receipt_is_an_additive_optional_field_not_a_new_grant_phase() {
    let sending = GrantRecord::sending(&request());
    let legacy = sending.to_json().unwrap();
    assert!(!legacy.contains("receipt"));
    assert_eq!(GrantRecord::from_json(&legacy).unwrap(), sending);
    let saved = sending.with_receipt(&receipt()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&saved.to_json().unwrap()).unwrap();
    assert_eq!(value["version"], 1);
    assert_eq!(value["phase"], "sending");
    assert!(value["result"].is_null());
    assert_eq!(value["receipt"]["kind"], "nousPortal");
    assert!(!format!("{:?}", saved.receipt()).contains("received-successor"));
}
