//! The receipt's original grant binding complements the record's exact revision/credential fence.
use super::{luma_tests::revision, *};
use crate::protocol::luma_oauth::ReceivedRefresh;
use serde_json::json;

#[test]
fn luma_received_receipt_cannot_rebind_context_refresh_or_recovery_identity() {
    let request = GrantRequest::from_revision(&revision(None), "luma-mcp", "saved-mcp").unwrap();
    let receipt = GrantReceipt::LumaOauth {
        tokens: ReceivedRefresh::capture(
            b"not-json",
            time::OffsetDateTime::now_utc(),
            request.material.luma_oauth.as_ref().unwrap(),
            &request.material.refresh,
        )
        .unwrap(),
    };
    let sending = GrantRecord::sending(&request);
    let saved = sending.with_receipt(&receipt).unwrap();
    assert_eq!(
        sending.identity_digest().unwrap(),
        saved.identity_digest().unwrap()
    );
    assert_eq!(
        GrantRecord::from_json(&saved.to_json().unwrap()).unwrap(),
        saved
    );
    assert_eq!(sending.with_receipt(&receipt).unwrap(), saved);
    let mut changed_anchor = serde_json::to_value(&receipt).unwrap();
    changed_anchor["tokens"]["startedAt"] = json!("2027-01-01T00:00:00Z");
    let changed_anchor: GrantReceipt = serde_json::from_value(changed_anchor).unwrap();
    assert!(saved.with_receipt(&changed_anchor).is_err());
    assert!(saved.replay_for(&request).is_none());
    assert!(!format!("{receipt:?}").contains("not-json"));
    for field in ["client", "redirect", "expiry", "refresh"] {
        let mut changed = request.clone();
        let mut raw = serde_json::to_value(&changed.material).unwrap();
        match field {
            "client" => {
                raw["client"] = json!("other-client");
                raw["lumaOauth"]["client_id"] = json!("other-client");
            }
            "redirect" => {
                raw["lumaOauth"]["redirect_uri"] = json!("http://127.0.0.1:54422/callback")
            }
            "expiry" => {
                raw["expiresAt"] = json!("2020-01-01T00:00:00Z");
                raw["lumaOauth"]["expires_at"] = raw["expiresAt"].clone();
            }
            "refresh" => raw["refresh"] = json!("other-refresh"),
            _ => unreachable!(),
        }
        changed.material = serde_json::from_value(raw).unwrap();
        changed.material.validate().unwrap();
        assert!(
            GrantRecord::sending(&changed)
                .with_receipt(&receipt)
                .is_err(),
            "{field}"
        );
        assert!(saved.receipt_for(&changed).is_none());
    }
    for field in ["provider", "credential", "revision"] {
        let mut changed = request.clone();
        match field {
            "provider" => changed.provider.push('x'),
            "credential" => changed.credential.push('x'),
            "revision" => changed.revision.push('x'),
            _ => unreachable!(),
        }
        assert!(saved.receipt_for(&changed).is_none());
    }
    let mut forged: serde_json::Value = serde_json::from_str(&saved.to_json().unwrap()).unwrap();
    forged["receipt"]["tokens"]["binding"] = json!("0".repeat(64));
    assert!(GrantRecord::from_json(&forged.to_string()).is_err());
}

#[test]
fn luma_received_valid_successor_still_cannot_become_result_or_published() {
    use crate::protocol::luma_oauth::AuthorizationContext;
    use time::format_description::well_known::Rfc3339;
    let request = GrantRequest::from_revision(&revision(None), "luma-mcp", "saved-mcp").unwrap();
    let original = request.material.luma_oauth.as_ref().unwrap();
    let tokens = ReceivedRefresh::capture(br#"{"access_token":"new-access","refresh_token":"new-refresh","token_type":"Bearer","expires_in":3600}"#,
        time::OffsetDateTime::now_utc(), original, &request.material.refresh).unwrap();
    let (material, refresh) = tokens
        .interpret_successor(original, &request.material.refresh)
        .unwrap()
        .into_parts();
    let mut next = request.material.clone();
    next.access = material.access_token().into();
    next.refresh = refresh;
    next.expires_at = Some(material.expires_at().unwrap().format(&Rfc3339).unwrap());
    next.luma_oauth = Some(AuthorizationContext::from_material(&material).unwrap());
    next.validate().unwrap();
    let result = GrantResult {
        expires_at: next.expires_at.clone(),
        material: next,
        expires_in: Some(3600),
    };
    let receipt = GrantReceipt::LumaOauth { tokens };
    assert!(receipt
        .validates_result(&request.material, &result)
        .is_err());
    let saved = GrantRecord::sending(&request)
        .with_receipt(&receipt)
        .unwrap();
    assert!(saved.with_result(&result).is_err());
    let mut forged: serde_json::Value = serde_json::from_str(&saved.to_json().unwrap()).unwrap();
    forged["result"] = serde_json::to_value(result).unwrap();
    for phase in ["result", "published"] {
        forged["phase"] = json!(phase);
        assert!(GrantRecord::from_json(&forged.to_string()).is_err());
    }
}

#[test]
fn luma_received_maximum_body_fits_the_existing_record_budget() {
    let request = GrantRequest::from_revision(&revision(None), "luma-mcp", "saved-mcp").unwrap();
    let receipt = GrantReceipt::LumaOauth {
        tokens: ReceivedRefresh::capture(
            &vec![255; 65536],
            time::OffsetDateTime::now_utc(),
            request.material.luma_oauth.as_ref().unwrap(),
            &request.material.refresh,
        )
        .unwrap(),
    };
    let saved = GrantRecord::sending(&request)
        .with_receipt(&receipt)
        .unwrap();
    let raw = saved.to_json().unwrap();
    assert!(raw.len() < MAX_GRANT_BYTES);
    assert_eq!(GrantRecord::from_json(&raw).unwrap(), saved);
}
