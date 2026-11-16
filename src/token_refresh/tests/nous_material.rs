//! Typed authority binds a receipt before Result and keeps old generic history unchanged.
use super::{nous_fixture::*, *};
use crate::console::refresh_grants::*;
use serde_json::json;

#[tokio::test]
async fn nous_typed_result_requires_receipt_and_inherits_the_old_welcome_target() {
    let f = fixture().await;
    let request = request(&f);
    let tokens = receipt();
    let result = result(&request, &tokens);
    let sending = GrantRecord::sending(&request);
    assert!(
        sending.with_result(&result).is_err(),
        "typed Result without receipt must be impossible"
    );
    let receipt = GrantReceipt::NousPortal { tokens };
    let received = sending.with_receipt(&receipt).unwrap();
    let accepted = received.with_result(&result).unwrap();
    assert_eq!(accepted.phase(), GrantPhase::Result);
    assert_eq!(
        result
            .material
            .nous_portal
            .as_ref()
            .unwrap()
            .inference_target(),
        crate::protocol::nous_portal::InferenceTarget::Welcome
    );
    let mut wrong = serde_json::to_value(&result).unwrap();
    wrong["material"]["nousPortal"]["inference"] = json!("subscription");
    let wrong = serde_json::from_value(wrong).unwrap();
    assert!(
        received.with_result(&wrong).is_err(),
        "Result context must match the captured receipt"
    );
    assert!(sending.replay_for(&request).is_none());
}

#[tokio::test]
async fn nous_material_rejects_mismatched_access_expiry_and_other_product_contexts() {
    let f = fixture().await;
    let request = request(&f);
    let material = request.material;
    assert!(material.validate().is_ok());
    for (pointer, value) in [
        ("/access", json!(access("other-subject", 4102444800))),
        ("/expiresAt", json!("2100-01-01T00:00:00.5Z")),
        ("/client", json!("foreign-client")),
        ("/endpoint", json!("https://example.com/token")),
        ("/nousPortal/access_digest", json!("b".repeat(64))),
    ] {
        let mut changed = serde_json::to_value(&material).unwrap();
        *changed.pointer_mut(pointer).unwrap() = value;
        let changed = serde_json::from_value(changed).unwrap();
        assert!(!material.permits(&changed), "{pointer}");
    }
    let mut other = material.clone();
    other.dim_oauth = Some(serde_json::from_value(super::dim_fixture::context()).unwrap());
    assert!(other.validate().is_err());
    let mut removed = material.clone();
    removed.nous_portal = None;
    assert!(!material.permits(&removed));
    assert!(!removed.permits(&material));
    assert_eq!(material.family_id(), removed.family_id());
    assert_ne!(material.material_id(), removed.material_id());
}

#[tokio::test]
async fn nous_absence_preserves_legacy_material_hash_and_generic_result_history() {
    use sha2::{Digest, Sha256};
    let f = LocalFixture::new(crate::protocol::nous_portal::TOKEN_ENDPOINT).await;
    let request = request(&f);
    let m = &request.material;
    assert!(serde_json::to_value(m).unwrap().get("nousPortal").is_none());
    let old = json!([
        "gateway-refresh-material-v1",
        m.endpoint,
        m.client,
        m.refresh,
        m.access,
        m.context,
        m.expires_at
    ]);
    assert_eq!(
        m.material_id(),
        hex::encode(Sha256::digest(old.to_string().as_bytes()))
    );
    let result = request
        .result("opaque-access", Some("opaque-refresh"), 3600, EXPIRY, None)
        .unwrap();
    let record = GrantRecord::sending(&request).with_result(&result).unwrap();
    assert!(GrantRecord::from_json(&record.to_json().unwrap()).is_ok());
}

#[tokio::test]
async fn nous_projection_and_authorization_identity_include_the_exact_context() {
    let f = fixture().await;
    let snapshot = f.runtime.route_config().snapshot();
    let before = snapshot.document();
    let request = request(&f);
    let result = result(&request, &receipt());
    let mut after = before.clone();
    let target = &mut after.providers[0].credentials[0];
    target.api_key = Some(result.material.access.clone());
    target.refresh_token = Some(result.material.refresh.clone());
    target.expires_at = result.expires_at.clone();
    target.token_expires_in_secs = result.expires_in;
    target.nous_portal = result.material.nous_portal.clone();
    assert!(!crate::routing::config::same_authorization(
        &before.providers[0].credentials[0],
        target
    ));
    assert!(is_refresh_projection(before, &after, "oauth", "account"));
    after.providers[0].credentials[0].expires_at = Some("2100-01-01T00:01:40.1Z".into());
    assert!(!is_refresh_projection(before, &after, "oauth", "account"));
}

#[tokio::test]
async fn nous_typed_receipt_and_result_roundtrip_through_the_real_local_authority() {
    use crate::console::RouteConfigRedisRevision;
    let f = fixture().await;
    let snapshot = f.runtime.route_config().snapshot();
    let revision =
        RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
            .unwrap();
    let request = request(&f);
    let GrantReservation::Reserved(sending) = f
        .runtime
        .reserve_refresh_grant(&revision, &request)
        .await
        .unwrap()
    else {
        panic!("fresh synthetic material must reserve once")
    };
    let tokens = receipt();
    let result = result(&request, &tokens);
    let receipt = GrantReceipt::NousPortal { tokens };
    let received = f
        .runtime
        .store_refresh_grant_receipt(&sending, &receipt)
        .await
        .unwrap();
    assert_eq!(received.phase(), GrantPhase::Sending);
    assert!(received.replay_for(&request).is_none());
    f.runtime
        .store_refresh_grant_result(&received, &result)
        .await
        .unwrap();
    let restarted = RouteConfigRuntime::new_local(&f.config).unwrap();
    restarted.recover_startup().await.unwrap();
    let GrantReservation::Existing(saved) = restarted
        .reserve_refresh_grant(&revision, &request)
        .await
        .unwrap()
    else {
        panic!("restart must retain original consumption evidence")
    };
    assert_eq!(saved.phase(), GrantPhase::Result);
    assert_eq!(saved.receipt(), Some(&receipt));
    assert!(saved
        .replay_for(&request)
        .is_some_and(|saved| saved == &result));
    assert_eq!(saved.material_id(), request.material_id());
}
