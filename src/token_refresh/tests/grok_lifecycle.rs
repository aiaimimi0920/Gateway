//! Context changes revoke stale owners; scope narrowing is part of rotated material, not advisory metadata.
use super::grok_wire::{fixture, refresh, rotated, Mock};
use super::*;
use serde_json::json;

async fn wait_called(mock: &Mock) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while mock.count() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn grok_context_change_retires_owner_and_blocks_late_result_without_changing_token_text() {
    for field in [
        "client_id",
        "principal",
        "requested_scopes",
        "granted_scopes",
    ] {
        let mock = Mock::new(200, rotated(), 100).await;
        let fixture = fixture().await;
        let initial = fixture.runtime.route_config().snapshot();
        let owner = initial
            .oauth_refresh_target("build-pool", "exact-build")
            .unwrap()
            .1
            .refresh_config
            .clone()
            .unwrap();
        let mut pending = Pending::new();
        let edit = async {
            wait_called(&mock).await;
            let snapshot = fixture.runtime.route_config().snapshot();
            let mut value = serde_json::to_value(snapshot.document()).unwrap();
            let context = &mut value["providers"][0]["credentials"][0]["grok_oauth"];
            context[field] = match field {
                "client_id" => json!("new-authorized-client"),
                "principal" => json!({"kind":"Team","id":"new-team"}),
                "requested_scopes" => json!(["openid", "profile", "email"]),
                _ => json!(["profile"]),
            };
            fixture
                .runtime
                .commit_document(
                    snapshot.revision().id(),
                    serde_json::from_value(value).unwrap(),
                    None,
                )
                .await
                .unwrap();
        };
        tokio::join!(refresh(&fixture, &mock, &mut pending), edit);
        assert!(!owner.is_active());
        assert!(pending.is_empty());
        assert_eq!(
            fixture.saved().api_key.as_deref(),
            Some("synthetic-build-token")
        );
        mock.stop().await;
    }
}

#[tokio::test]
async fn grok_concurrent_refresh_is_single_flight_and_no_rotation_retains_refresh_token() {
    let mock = Mock::new(
        200,
        json!({"access_token":"new-access","expires_in":3600}),
        50,
    )
    .await;
    let fixture = fixture().await;
    let (mut a, mut b) = (Pending::new(), Pending::new());
    tokio::join!(
        refresh(&fixture, &mock, &mut a),
        refresh(&fixture, &mock, &mut b)
    );
    assert_eq!(mock.count(), 1);
    assert_eq!(
        fixture.saved().refresh_token.as_deref(),
        Some("synthetic-refresh")
    );
    assert!(a.is_empty() && b.is_empty());
    mock.stop().await;
}

#[tokio::test]
async fn grok_pending_rotation_and_scope_narrowing_retry_disk_only() {
    let mut reply = rotated();
    reply["scope"] = json!("profile");
    let mock = Mock::new(200, reply, 100).await;
    let fixture = fixture().await;
    let mut pending = Pending::new();
    let (_, guard) = tokio::join!(refresh(&fixture, &mock, &mut pending), async {
        wait_called(&mock).await;
        fixture.writer_lock()
    });
    assert_eq!(pending.len(), 1);
    let snapshot = fixture.runtime.route_config().snapshot();
    let config = snapshot
        .oauth_refresh_target("build-pool", "exact-build")
        .unwrap()
        .1
        .refresh_config
        .as_ref()
        .unwrap();
    let value = serde_json::to_value(config.authorization_context()).unwrap();
    assert_eq!(
        value["granted_scopes"],
        json!(["openid", "profile"]),
        "scope changes are not applied before durable Result storage"
    );
    refresh(&fixture, &mock, &mut pending).await;
    assert_eq!(mock.count(), 1);
    drop(guard);
    refresh(&fixture, &mock, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(mock.count(), 1);
    assert_eq!(
        serde_json::to_value(fixture.saved().grok_oauth).unwrap()["granted_scopes"],
        json!(["profile"])
    );
    let restarted = RouteConfigRuntime::new_local(&fixture.config).unwrap();
    restarted.recover_startup().await.unwrap();
    assert_eq!(
        serde_json::to_value(
            &restarted.route_config().snapshot().document().providers[0].credentials[0].grok_oauth
        )
        .unwrap()["granted_scopes"],
        json!(["profile"])
    );
    mock.stop().await;
}

#[tokio::test]
async fn grok_scope_expansion_after_observed_narrowing_is_rejected_even_when_initial_grant_was_unknown(
) {
    let fixture = fixture().await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut value = serde_json::to_value(snapshot.document()).unwrap();
    value["providers"][0]["credentials"][0]["grok_oauth"]
        .as_object_mut()
        .unwrap()
        .remove("granted_scopes");
    fixture
        .runtime
        .commit_document(
            snapshot.revision().id(),
            serde_json::from_value(value).unwrap(),
            None,
        )
        .await
        .unwrap();
    let mut reply = rotated();
    reply["scope"] = json!("profile");
    let narrow = Mock::new(200, reply, 0).await;
    refresh(&fixture, &narrow, &mut Pending::new()).await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let owner = snapshot
        .oauth_refresh_target("build-pool", "exact-build")
        .unwrap()
        .1
        .refresh_config
        .clone()
        .unwrap();
    owner.force_due();
    let expanded = Mock::new(
        200,
        json!({"access_token":"expanded-access","expires_in":3600,"scope":"openid profile"}),
        0,
    )
    .await;
    refresh(&fixture, &expanded, &mut Pending::new()).await;
    assert_eq!(expanded.count(), 1);
    assert_eq!(
        fixture.saved().api_key.as_deref(),
        Some("rotated-build-access")
    );
    assert_eq!(
        serde_json::to_value(fixture.saved().grok_oauth).unwrap()["granted_scopes"],
        json!(["profile"])
    );
    narrow.stop().await;
    expanded.stop().await;
}
