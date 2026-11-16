//! Independent cache reassignment and same-token expiry renewal are not old publication targets.
use super::{cache_recovery::refresh, cache_wire::Cache, grok_wire::Mock, *};
use serde_json::json;

#[tokio::test]
async fn grok_repair_rejects_changed_cache_identity_product_client_endpoint_and_expiry() {
    let authority = authority_fixture::Authority::new();
    let mock = Mock::new(200, grok_wire::rotated(), 0).await;
    let cache = Cache::new(false).await;
    authority.backend.bind_cache(&cache.pool);
    let original = cache.value();
    let mut pending = Pending::new();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    let record = &pending.values().next().unwrap().as_result().unwrap().record;
    let snapshot = authority.runtime.route_config().snapshot();
    let (_, credential) = snapshot
        .oauth_refresh_target("build-pool", "exact-build")
        .unwrap();
    assert!(super::super::server_cache::patch_from_grant(
        &original.to_string(),
        record,
        &credential.payload
    )
    .unwrap()
    .is_some());
    for path in [
        "/id",
        "/api_base_url",
        "/account_payload/preset",
        "/account_payload/refresh_endpoint",
        "/account_payload/refresh_client_id",
        "/expires_at",
    ] {
        let mut reassigned = original.clone();
        match path {
            "/id" => reassigned["id"] = json!("another-credential"),
            "/api_base_url" => reassigned["api_base_url"] = json!("https://other.invalid/v1"),
            "/account_payload/preset" => reassigned["account_payload"]["preset"] = json!("xai-api"),
            "/account_payload/refresh_endpoint" => {
                reassigned["account_payload"]["refresh_endpoint"] =
                    json!("https://other.invalid/token")
            }
            "/account_payload/refresh_client_id" => {
                reassigned["account_payload"]["refresh_client_id"] = json!("another-client")
            }
            "/expires_at" => reassigned["expires_at"] = json!("2099-01-01T00:00:00Z"),
            _ => unreachable!(),
        }
        assert!(
            super::super::server_cache::patch_from_grant(
                &reassigned.to_string(),
                record,
                &credential.payload
            )
            .unwrap()
            .is_none(),
            "{path}"
        );
    }
    for field in ["refresh_endpoint", "refresh_client_id", "expires_at"] {
        for invalid in [json!(123), json!({}), json!(false), json!([])] {
            let mut malformed = original.clone();
            malformed["account_payload"][field] = invalid;
            assert!(
                super::super::server_cache::patch_from_grant(
                    &malformed.to_string(),
                    record,
                    &credential.payload
                )
                .unwrap()
                .is_none(),
                "malformed {field}"
            );
        }
    }
    let mut optional_null = original.clone();
    for field in ["refresh_endpoint", "refresh_client_id", "expires_at"] {
        optional_null["account_payload"][field] = serde_json::Value::Null;
    }
    assert!(
        super::super::server_cache::patch_from_grant(
            &optional_null.to_string(),
            record,
            &credential.payload
        )
        .unwrap()
        .is_some(),
        "optional null retains the missing-field semantics"
    );
    let legacy = json!({"api_key":"synthetic-build-token","account_payload":{
        "refresh_token":"synthetic-refresh","grok_oauth":grok_context::context()}});
    assert!(
        super::super::server_cache::patch_from_grant(
            &legacy.to_string(),
            record,
            &credential.payload
        )
        .unwrap()
        .is_none(),
        "unattributable legacy data is not silently accepted as a hosted credential"
    );
    let mut missing_product = original.clone();
    missing_product["account_payload"]
        .as_object_mut()
        .unwrap()
        .remove("preset");
    assert!(
        super::super::server_cache::patch_from_grant(
            &missing_product.to_string(),
            record,
            &credential.payload
        )
        .unwrap()
        .is_none(),
        "Build product ownership is not inferred from shared adapter or tokens"
    );
    mock.stop().await;
    cache.stop().await;
}

#[test]
fn generic_repair_rejects_ill_typed_missing_client_and_unknown_payload_expiry() {
    use crate::console::{refresh_grants::*, RouteConfigRedisRevision};
    let store = RouteConfigStore::from_document(
        serde_json::from_value(json!({
            "providers":[{"id":"oauth","base_url":"https://example.com/v1","credentials":[{
                "id":"account","api_key":"old-access","refresh_token":"old-refresh",
                "refresh_endpoint":"https://example.com/token"
            }]}]
        }))
        .unwrap(),
    )
    .unwrap();
    let snapshot = store.snapshot();
    let revision =
        RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
            .unwrap();
    let request = GrantRequest::from_revision(&revision, "oauth", "account").unwrap();
    let record = GrantRecord::sending(&request);
    let (_, credential) = snapshot.oauth_refresh_target("oauth", "account").unwrap();
    let original = json!({"id":"account","kind":"account_credential","project_id":"p","user_id":"u",
        "provider":"openai","api_key":"old-access","api_base_url":"https://example.com/v1",
        "created_at":"2020-01-01T00:00:00Z","updated_at":"2020-01-01T00:00:00Z",
        "account_payload":{"refresh_token":"old-refresh","refresh_endpoint":"https://example.com/token"}});
    for field in ["refresh_client_id", "expires_at"] {
        for absent in [false, true] {
            let mut compatible = original.clone();
            if !absent {
                compatible["account_payload"][field] = serde_json::Value::Null;
            }
            assert!(super::super::cache_target::matches(
                &compatible.to_string(),
                &record,
                &credential.payload
            ));
        }
        for invalid in [json!(123), json!({}), json!(false), json!([])] {
            let mut malformed = original.clone();
            malformed["account_payload"][field] = invalid;
            assert!(
                !super::super::cache_target::matches(
                    &malformed.to_string(),
                    &record,
                    &credential.payload
                ),
                "malformed {field}"
            );
        }
    }
}

#[tokio::test]
async fn generic_repair_requires_the_original_refresh_endpoint_client_and_absolute_expiry() {
    use crate::console::{refresh_grants::*, RouteConfigRedisRevision};
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let revision =
        RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
            .unwrap();
    let request = GrantRequest::from_revision(&revision, "oauth", "account").unwrap();
    let GrantReservation::Reserved(record) = fixture
        .runtime
        .reserve_refresh_grant(&revision, &request)
        .await
        .unwrap()
    else {
        panic!("new synthetic grant");
    };
    let result = request
        .result("new", Some("new-rt"), 3600, "2099-01-01T00:00:00Z", None)
        .unwrap();
    let record = fixture
        .runtime
        .store_refresh_grant_result(&record, &result)
        .await
        .unwrap();
    let (_, credential) = snapshot.oauth_refresh_target("oauth", "account").unwrap();
    let original = json!({"id":"account","kind":"account_credential","project_id":"p","user_id":"u",
        "provider":"openai","api_key":"old-access","api_base_url":"https://example.com/v1",
        "created_at":"2020-01-01T00:00:00Z","updated_at":"2020-01-01T00:00:00Z","expires_at":"2020-01-01T00:00:00Z",
        "account_payload":{"refresh_token":"old-refresh","refresh_endpoint":mock.url,"refresh_client_id":"fixture-client"}});
    assert!(super::super::server_cache::patch_from_grant(
        &original.to_string(),
        &record,
        &credential.payload
    )
    .unwrap()
    .is_some());
    for field in ["refresh_endpoint", "refresh_client_id", "refresh_token"] {
        let mut value = original.clone();
        value["account_payload"][field] = json!("https://other.invalid/token");
        assert!(super::super::server_cache::patch_from_grant(
            &value.to_string(),
            &record,
            &credential.payload
        )
        .unwrap()
        .is_none());
    }
    assert_eq!(mock.count(), 0, "target checks are offline");
}

#[tokio::test]
async fn server_writer_lock_before_recovery_defers_the_exchange_without_pending_or_cache_write() {
    let authority = authority_fixture::Authority::new();
    let mock = Mock::new(200, grok_wire::rotated(), 0).await;
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let guard = authority.persistence.try_writer_lock().unwrap();
    let mut pending = Pending::new();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(mock.count(), 0);
    assert_eq!(cache.commits(), 0);
    drop(guard);
    mock.stop().await;
    cache.stop().await;
}
