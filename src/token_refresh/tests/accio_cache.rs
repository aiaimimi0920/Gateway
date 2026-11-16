//! Normal cache writeback and restart patch compare the complete previous SG, never caller payload.
use super::{accio_fixture::*, *};
use crate::console::refresh_grants::GrantRecord;
use serde_json::{json, Value};

#[tokio::test]
async fn accio_cache_repair_persists_effective_cookie_and_rejects_reassignment_or_bad_types() {
    for previous in [None, Some("old-cookie")] {
        let fixture = fixture(previous).await;
        let request = request(&fixture).1;
        let result = request
            .result_with_accio(
                "new-access",
                Some("new-refresh"),
                1,
                "2021-01-01T00:00:00Z",
                None,
                Some("new-cookie".into()),
            )
            .unwrap();
        let record = GrantRecord::sending(&request).with_result(&result).unwrap();
        let snapshot = fixture.runtime.route_config().snapshot();
        let target = &snapshot
            .oauth_refresh_target("oauth", "account")
            .unwrap()
            .1
            .payload;
        let patch = |value: &Value| {
            super::super::server_cache::patch_from_grant(&value.to_string(), &record, target)
                .unwrap()
        };
        let original = cache(previous);
        let (patched, ttl) = patch(&original).unwrap();
        let patched: Value = serde_json::from_str(&patched).unwrap();
        assert_eq!(
            ttl, 1,
            "cache TTL cannot replace an expired absolute authorization"
        );
        assert_eq!(patched["account_payload"]["accio_sgcookie"], "new-cookie");
        assert_eq!(patched["expires_at"], "2021-01-01T00:00:00Z");
        assert_eq!(patched["quota_remaining_tokens"], 42);
        assert_eq!(patched["unknown"], "retained");
        assert!(patch(&patched).is_none());
        for value in [
            json!("other-cookie"),
            json!(123),
            json!(false),
            json!({}),
            json!([]),
        ] {
            let mut changed = original.clone();
            changed["account_payload"]["accio_sgcookie"] = value;
            assert!(patch(&changed).is_none());
        }
        let mut null = original.clone();
        null["account_payload"]["accio_sgcookie"] = Value::Null;
        assert_eq!(patch(&null).is_some(), previous.is_none());
        let mut alias = original.clone();
        alias["account_payload"]["accioSgcookie"] = json!("other-cookie");
        assert!(
            patch(&alias).is_none(),
            "a contradictory alias must not be erased as missing"
        );
    }
}

#[tokio::test]
async fn accio_normal_cache_wire_cas_binds_old_cookie_and_writes_final_material() {
    let witness = super::cache_wire::Cache::new(true).await;
    let mut raw = cache(Some("old-cookie"));
    raw["id"] = json!("exact-build");
    witness.replace_raw(raw.clone());
    let token = super::super::transport::TokenResponse {
        access_token: "new-access".into(),
        refresh_token: Some("new-refresh".into()),
        expires_in: Some(3600),
        grok_oauth: None,
        absolute_expiry: Some("2099-01-01T00:00:00Z".into()),
        accio_sgcookie: Some("new-cookie".into()),
        vertex_oauth: None,
        dim_oauth: None,
        nous_portal: None,
        lobsterai: None,
    };
    super::super::server_cache::persist(
        &witness.pool,
        "exact-build",
        "old-access",
        "old-refresh",
        None,
        None,
        None,
        None,
        Some("other-cookie"),
        &token,
        "2099-01-01T00:00:00Z",
    )
    .await
    .unwrap();
    assert_eq!(witness.commits(), 0);
    assert_eq!(witness.value(), raw);
    super::super::server_cache::persist(
        &witness.pool,
        "exact-build",
        "old-access",
        "old-refresh",
        None,
        None,
        None,
        None,
        Some("old-cookie"),
        &token,
        "2099-01-01T00:00:00Z",
    )
    .await
    .unwrap();
    assert_eq!(witness.commits(), 1);
    assert_eq!(
        witness.value()["account_payload"]["accio_sgcookie"],
        "new-cookie"
    );
    assert_eq!(witness.value()["quota_remaining_tokens"], 42);
    witness.stop().await;
}
