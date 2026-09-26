use std::task::Poll;
use std::time::Duration;

use neuro_gateway::provider_credential_folder_sync::{
    get_folder_sync_status, load_runtime_enabled, set_runtime_enabled,
};
use neuro_gateway::redis::keys;
use redis::AsyncCommands;

use crate::fixture::Fixture;

async fn enabled_override(fixture: &Fixture) -> Option<String> {
    fixture
        .state
        .redis_pool
        .get()
        .await
        .unwrap()
        .get(keys::provider_credential_folder_sync_enabled_key())
        .await
        .unwrap()
}

async fn write_enabled_override_without_event(fixture: &Fixture, enabled: bool) {
    let mut connection = fixture.state.redis_pool.get().await.unwrap();
    let _: () = connection
        .set(
            keys::provider_credential_folder_sync_enabled_key(),
            enabled.to_string(),
        )
        .await
        .unwrap();
}

async fn assert_enabled(fixture: &Fixture, expected: bool) {
    assert_eq!(
        fixture.state.provider_credential_folder_sync.enabled(),
        expected
    );
    assert_eq!(enabled_override(fixture).await, Some(expected.to_string()));
    assert_eq!(fixture.status().await.unwrap().enabled, expected);
}

async fn pubsub_client_ids(fixture: &Fixture) -> Vec<String> {
    let mut connection = fixture.state.redis_pool.get().await.unwrap();
    let clients: String = redis::cmd("CLIENT")
        .arg("LIST")
        .arg("TYPE")
        .arg("pubsub")
        .query_async(&mut connection)
        .await
        .unwrap();
    clients
        .lines()
        .filter_map(|line| {
            line.split_whitespace()
                .find_map(|field| field.strip_prefix("id="))
                .map(str::to_owned)
        })
        .collect()
}

async fn drop_pubsub_connections(fixture: &Fixture) {
    let ids = pubsub_client_ids(fixture).await;
    assert!(
        !ids.is_empty(),
        "folder sync listeners must have pubsub clients"
    );
    let mut connection = fixture.state.redis_pool.get().await.unwrap();
    for id in ids {
        let _: i64 = redis::cmd("CLIENT")
            .arg("KILL")
            .arg("ID")
            .arg(id)
            .query_async(&mut connection)
            .await
            .unwrap();
    }
}

async fn wait_for_pubsub_clients(fixture: &Fixture, expected: usize) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if pubsub_client_ids(fixture).await.len() >= expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("folder sync listeners must establish pubsub clients");
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_admitted_enable_survives_caller_cancellation() {
    let fixture = Fixture::new(false, false, true).await;
    fixture.state.redis_pool.resize(1);
    let connection = fixture.state.redis_pool.get().await.unwrap();
    let mut request = Box::pin(set_runtime_enabled(&fixture.state, true));
    // The uncontended permit is acquired on this poll; Redis cannot yet be used.
    assert!(matches!(futures::poll!(request.as_mut()), Poll::Pending));
    assert!(!fixture.state.provider_credential_folder_sync.enabled());
    drop(request);
    drop(connection);
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if fixture.status().await.is_some_and(|status| status.enabled)
                && fixture.state.provider_credential_folder_sync.enabled()
            {
                assert_enabled(&fixture, true).await;
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("admitted enable completes after the caller is dropped");
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_cancelled_enable_waiter_does_not_mutate_state() {
    let fixture = Fixture::new(false, false, true).await;
    fixture.state.redis_pool.resize(1);
    let connection = fixture.state.redis_pool.get().await.unwrap();
    let mut first = Box::pin(set_runtime_enabled(&fixture.state, true));
    assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));
    let mut queued = Box::pin(set_runtime_enabled(&fixture.state, false));
    assert!(matches!(futures::poll!(queued.as_mut()), Poll::Pending));
    drop(queued);
    drop(connection);
    tokio::time::timeout(Duration::from_secs(3), async {
        assert!(first.await.unwrap().enabled);
        assert_enabled(&fixture, true).await;
        // A subsequent request also proves that cancellation released its wait slot.
        assert!(
            !set_runtime_enabled(&fixture.state, false)
                .await
                .unwrap()
                .enabled
        );
        assert_enabled(&fixture, false).await;
    })
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_enable_requests_commit_in_admission_order() {
    let fixture = Fixture::new(false, false, true).await;
    fixture.state.redis_pool.resize(1);
    let connection = fixture.state.redis_pool.get().await.unwrap();
    let mut first = Box::pin(set_runtime_enabled(&fixture.state, true));
    assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));
    let mut second = Box::pin(set_runtime_enabled(&fixture.state, false));
    assert!(matches!(futures::poll!(second.as_mut()), Poll::Pending));
    drop(connection);
    tokio::time::timeout(Duration::from_secs(3), async {
        // Both remain polled: a queued pool reservation must not starve its peer.
        let (first, second) = tokio::join!(first, second);
        assert!(first.unwrap().enabled);
        assert!(!second.unwrap().enabled);
        assert_enabled(&fixture, false).await;
    })
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_status_error_releases_enable_admission_without_partial_write() {
    let fixture = Fixture::new(false, false, true).await;
    let key = keys::provider_credential_folder_sync_status_key();
    let mut connection = fixture.state.redis_pool.get().await.unwrap();
    let _: () = connection.set(&key, "{broken").await.unwrap();
    drop(connection);
    tokio::time::timeout(Duration::from_secs(3), async {
        for enabled in [true, false] {
            let error = set_runtime_enabled(&fixture.state, enabled)
                .await
                .unwrap_err();
            assert!(error.message.starts_with("decode folder sync status:"));
            assert!(!fixture.state.provider_credential_folder_sync.enabled());
            assert_eq!(enabled_override(&fixture).await, None);
            let mut connection = fixture.state.redis_pool.get().await.unwrap();
            let raw: String = connection.get(&key).await.unwrap();
            assert_eq!(
                raw, "{broken",
                "status failure leaves the malformed snapshot untouched"
            );
        }
        let mut connection = fixture.state.redis_pool.get().await.unwrap();
        let _: () = connection.set(&key, "{}").await.unwrap();
        drop(connection);
        assert!(
            set_runtime_enabled(&fixture.state, true)
                .await
                .unwrap()
                .enabled
        );
        assert_enabled(&fixture, true).await;
    })
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_status_reads_shared_enabled_override_without_mutating_local_runtime() {
    let fixture = Fixture::new(false, false, true).await;
    let mut connection = fixture.state.redis_pool.get().await.unwrap();
    let _: () = connection
        .set(keys::provider_credential_folder_sync_enabled_key(), "true")
        .await
        .unwrap();
    drop(connection);

    let status = get_folder_sync_status(&fixture.state).await.unwrap();

    assert!(status.enabled);
    assert!(!fixture.state.provider_credential_folder_sync.enabled());
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_startup_enabled_override_ignores_malformed_status() {
    let fixture = Fixture::new(false, false, true).await;
    let mut connection = fixture.state.redis_pool.get().await.unwrap();
    let _: () = connection
        .set(
            keys::provider_credential_folder_sync_status_key(),
            "{broken",
        )
        .await
        .unwrap();
    let _: () = connection
        .set(keys::provider_credential_folder_sync_enabled_key(), "true")
        .await
        .unwrap();
    drop(connection);

    assert!(
        load_runtime_enabled(&fixture.state.redis_pool, &fixture.state.config)
            .await
            .unwrap()
    );
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_enabled_control_converges_across_runtime_tasks() {
    let mut first = Fixture::new(false, false, true).await;
    let mut second = Fixture::new(false, false, true).await;
    first.start();
    second.start();
    first.wait_disabled_status().await;
    second.wait_disabled_status().await;
    wait_for_pubsub_clients(&first, 2).await;

    set_runtime_enabled(&first.state, true).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if second.state.provider_credential_folder_sync.enabled() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("second runtime receives enabled control event");
    second.wait_root().await;

    set_runtime_enabled(&first.state, false).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if !second.state.provider_credential_folder_sync.enabled() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("second runtime receives disabled control event");

    first.finish_gracefully().await;
    second.finish_gracefully().await;
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_enabled_control_reconciles_after_pubsub_disconnect() {
    let mut first = Fixture::new(false, false, true).await;
    let mut second = Fixture::new(false, false, true).await;
    first.start();
    second.start();
    first.wait_disabled_status().await;
    second.wait_disabled_status().await;
    wait_for_pubsub_clients(&first, 2).await;

    set_runtime_enabled(&first.state, true).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if second.state.provider_credential_folder_sync.enabled() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("second runtime receives enabled control event");
    second.wait_root().await;

    drop_pubsub_connections(&first).await;
    assert!(first.state.provider_credential_folder_sync.enabled());
    // Bypass the management setter so no Pub/Sub event can satisfy this test.
    write_enabled_override_without_event(&first, false).await;
    assert_eq!(enabled_override(&first).await, Some("false".to_owned()));

    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if !second.state.provider_credential_folder_sync.enabled() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("second runtime reconciles the override after reconnect");

    first.finish_gracefully().await;
    second.finish_gracefully().await;
}
