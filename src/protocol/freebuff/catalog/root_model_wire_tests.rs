//! Real loopback catalog -> immutable selection -> business root input; not inference or a full root.
use super::{
    fixture::*,
    tests::body,
    wire_tests::{fetch, key, session},
    *,
};
use serde_json::json;
use std::time::Duration;
const BUDGET: Duration = Duration::from_secs(3);

fn catalog(label: &str, digest: &str, lean: bool) -> serde_json::Value {
    let mut value = body();
    value["fetchId"] = json!(format!("fetch-{label}"));
    value["rows"][0]["handle"] = json!(format!("fbm1.{label}"));
    value["rows"][0]["displayName"] = json!(label);
    value["rows"][0]["legacyDigests"] = json!([digest]);
    value["rows"][0]["compaction"] = json!({"cacheExpiryMs":900000,
        "cacheExpiryMinTokens":40000,"maxContextLength":110000,"lean":lean});
    value
}

#[tokio::test]
async fn freebuff_root_model_wire_refresh_pins_all_inputs_and_malformed_policy_clears_snapshot() {
    let first = catalog("first", "1e303ac563a6f9cc", false);
    let second = catalog("second", "957291675c9f9132", true);
    let mut invalid = second.clone();
    invalid["rows"][0]["compaction"]["cacheExpiryMs"] = json!(0);
    let peer = Peer::start(vec![
        Spec::json(200, first),
        Spec::json(200, json!({"keyId":"synthetic-key"})),
        Spec::json(200, second),
        Spec::json(200, invalid),
    ])
    .await;
    let mut owner = session(ClientSurface::Cli);
    fetch(&peer, &mut owner, None, BUDGET).await.unwrap();
    let device = TEST_ORIGIN
        .scope(peer.origin.clone(), owner.register_device(key().0, BUDGET))
        .await
        .unwrap();
    {
        let requests = owner.business_requests(&device, "open").unwrap();
        let root = requests.root_model_input();
        assert_eq!((root.model, root.model_label), ("fbm1.first", "first"));
        assert_eq!(requests.selected.fetch_id, Some("fetch-first"));
        assert_eq!(root.compiled_model_id, Some("deepseek/deepseek-v4-flash"));
        assert!(root.batching && !root.turn_ending_note);
        assert_eq!(root.compaction.unwrap().lean, Some(false));
        assert_eq!(
            peer.count(),
            2,
            "local model input neither purchases nor sends inference"
        );
    }
    fetch(&peer, &mut owner, Some(&device), BUDGET)
        .await
        .unwrap();
    {
        let requests = owner.business_requests(&device, "open").unwrap();
        let root = requests.root_model_input();
        assert_eq!((root.model, root.model_label), ("fbm1.second", "second"));
        assert_eq!(requests.selected.fetch_id, Some("fetch-second"));
        assert_eq!(root.compiled_model_id, Some("upstage/solar-mini4"));
        assert!(!root.batching && root.turn_ending_note);
        assert_eq!(root.compaction.unwrap().lean, Some(true));
    }
    assert_eq!(
        fetch(&peer, &mut owner, Some(&device), BUDGET).await,
        Err(CatalogError::InvalidResponse)
    );
    assert!(owner.catalog().is_none());
    assert_eq!(
        owner.business_requests(&device, "open").err(),
        Some(BusinessError::Catalog(CatalogError::CatalogRequired))
    );
    assert_eq!(
        peer.count(),
        4,
        "no fallback fetch or admission after invalid policy"
    );
    assert!(peer.hits.lock().unwrap()[2]
        .headers
        .get("x-freebuff-device-sig")
        .is_some());
    peer.close().await;
}

#[tokio::test]
async fn freebuff_root_model_wire_transient_refresh_keeps_original_inputs_without_reconstruction() {
    let peer = Peer::start(vec![
        Spec::json(200, catalog("original", "1e775c266d0065f1", true)),
        Spec::json(200, json!({"keyId":"synthetic-key"})),
        Spec::json(429, json!({})),
    ])
    .await;
    let mut owner = session(ClientSurface::Cli);
    fetch(&peer, &mut owner, None, BUDGET).await.unwrap();
    let device = TEST_ORIGIN
        .scope(peer.origin.clone(), owner.register_device(key().0, BUDGET))
        .await
        .unwrap();
    assert_eq!(
        fetch(&peer, &mut owner, Some(&device), BUDGET).await,
        Err(CatalogError::Retryable)
    );
    let requests = owner.business_requests(&device, "open").unwrap();
    let root = requests.root_model_input();
    assert_eq!(
        (root.model, root.model_label),
        ("fbm1.original", "original")
    );
    assert_eq!(requests.selected.fetch_id, Some("fetch-original"));
    assert_eq!(
        root.compiled_model_id,
        Some("deepseek/deepseek-v4-flash-fast")
    );
    assert_eq!(root.compaction.unwrap().cache_expiry_ms, 900000);
    assert!(!root.batching && !root.turn_ending_note);
    assert_eq!(
        peer.count(),
        3,
        "no automatic retry, policy fetch, START or purchase"
    );
    peer.close().await;
}
