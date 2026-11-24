//! Real local runtime startup errors must release the same owners as normal HTTP exit.
use super::*;
use crate::console::{ConsoleConfig, ConsoleConfigValues};
use std::time::Duration;

#[path = "../tests/pipeline_send_runtime/config.rs"]
mod fixture_config;

async fn local_state() -> Arc<AppState> {
    // The evidence runner places TEMP/TMP under linshi; retain fixtures for inspection.
    let root =
        std::env::temp_dir().join(format!("gateway-local-lifecycle-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&root).await.unwrap();
    tokio::fs::write(
        root.join("routes.yaml"),
        "providers: []\nmodel_routes: []\n",
    )
    .await
    .unwrap();
    let console = ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(root.join("state")),
        routes_file: Some(root.join("routes.yaml")),
        redis_namespace: Some("runtime-lifecycle".into()),
        ..Default::default()
    })
    .unwrap();
    let mut config = fixture_config::test_config(console);
    config.storage_mode = GatewayStorageMode::Local;
    build_app_state(config).await.unwrap()
}

#[tokio::test]
async fn binding_failure_requests_shutdown_and_closes_local_storage() {
    let host = std::env::var("GATEWAY_BIND_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let occupied = tokio::net::TcpListener::bind((host.as_str(), 0))
        .await
        .unwrap();
    let state = local_state().await;
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        run_gateway_server(state.clone(), occupied.local_addr().unwrap().port()),
    )
    .await;
    let requested = state.shutdown.is_requested();
    let closed = state.local_runtime.as_ref().unwrap().pool.is_closed();
    // Even the red test releases its synthetic resources before making assertions.
    state.shutdown.request("test cleanup");
    if !closed {
        state.local_runtime.as_ref().unwrap().close().await;
    }
    assert!(result
        .expect("binding failure must finish promptly")
        .is_err());
    assert!(requested, "bind error bypassed runtime shutdown");
    assert!(closed, "bind error bypassed local close");
}

#[tokio::test]
async fn successful_http_exit_closes_local_storage() {
    let state = local_state().await;
    state.shutdown.request("synthetic normal exit");
    tokio::time::timeout(Duration::from_secs(5), run_gateway_server(state.clone(), 0))
        .await
        .unwrap()
        .unwrap();
    assert!(state.local_runtime.as_ref().unwrap().pool.is_closed());
}

#[tokio::test]
async fn normal_background_bootstrap_is_deduplicated_and_binding_failure_joins_every_worker() {
    // Windows can bind wildcard and loopback listeners to the same port.
    let host = std::env::var("GATEWAY_BIND_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let occupied = tokio::net::TcpListener::bind((host.as_str(), 0))
        .await
        .unwrap();
    let state = local_state().await;
    spawn_background_tasks(&state);
    spawn_background_tasks(&state);
    assert_eq!(
        state.lifecycle.background_tasks.workers(),
        vec![
            Worker::NetworkProbe,
            Worker::LocalMaintenance,
            Worker::LumaReconciliation,
            Worker::SapResponseRecovery,
            Worker::CredentialRefill,
            Worker::CredentialProbe,
        ]
    );
    let error = tokio::time::timeout(
        Duration::from_secs(5),
        run_gateway_server(state.clone(), occupied.local_addr().unwrap().port()),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(error.downcast_ref::<std::io::Error>().is_some());
    assert!(state.lifecycle.background_tasks.workers().is_empty());
    assert!(state.local_runtime.as_ref().unwrap().pool.is_closed());
    // An already stopped runtime cannot register another bootstrap or router worker.
    spawn_background_tasks(&state);
    assert!(state.lifecycle.background_tasks.workers().is_empty());
}

#[tokio::test]
async fn binding_failure_drains_admitted_write_and_preserves_bind_error_before_local_retirement() {
    let host = std::env::var("GATEWAY_BIND_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let occupied = tokio::net::TcpListener::bind((host.as_str(), 0))
        .await
        .unwrap();
    let state = local_state().await;
    let local = state.local_runtime.as_ref().unwrap().clone();
    let old_owner = local.owner.clone();
    let (release, wait) = tokio::sync::oneshot::channel();
    let (started, ready) = tokio::sync::oneshot::channel();
    assert!(state.lifecycle.background_tasks.spawn(
        Worker::LocalMaintenance,
        &state.shutdown,
        async move {
            // Hold an actually begun writer transaction across HTTP shutdown.
            let mut tx = local.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
            sqlx::query("CREATE TABLE runtime_lifecycle_test_commit (id INTEGER PRIMARY KEY)")
                .execute(&mut *tx)
                .await
                .unwrap();
            sqlx::query("INSERT INTO runtime_lifecycle_test_commit (id) VALUES (1)")
                .execute(&mut *tx)
                .await
                .unwrap();
            started.send(()).unwrap();
            wait.await.unwrap();
            tx.commit().await.unwrap();
            local.maintain().await.unwrap();
        }
    ));
    ready.await.unwrap();
    assert!(state.lifecycle.background_tasks.spawn(
        Worker::CredentialProbe,
        &state.shutdown,
        async { panic!("synthetic background failure") }
    ));
    let mut server = tokio::spawn(run_gateway_server(
        state.clone(),
        occupied.local_addr().unwrap().port(),
    ));
    assert!(tokio::time::timeout(Duration::from_millis(50), &mut server)
        .await
        .is_err());
    assert!(!state.local_runtime.as_ref().unwrap().pool.is_closed());
    release.send(()).unwrap();
    let error = tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(error.downcast_ref::<std::io::Error>().is_some());
    assert!(state.local_runtime.as_ref().unwrap().pool.is_closed());
    let reopened = crate::local_runtime::LocalRuntime::open(&state.config.console.state_dir)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_instances WHERE id = ?")
        .bind(old_owner)
        .fetch_one(&reopened.pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "maintenance reinserted an already retired runtime"
    );
    let committed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_lifecycle_test_commit")
        .fetch_one(&reopened.pool)
        .await
        .unwrap();
    assert_eq!(committed, 1, "shutdown discarded an admitted transaction");
    reopened.close().await;
}

#[tokio::test]
async fn background_failure_after_successful_http_exit_still_closes_local_storage() {
    let state = local_state().await;
    assert!(state.lifecycle.background_tasks.spawn(
        Worker::CredentialProbe,
        &state.shutdown,
        async { panic!("synthetic background failure") }
    ));
    state.shutdown.request("synthetic successful serve");
    let error = run_gateway_server(state.clone(), 0).await.unwrap_err();
    assert!(error.to_string().contains("CredentialProbe: panicked"));
    assert!(state.local_runtime.as_ref().unwrap().pool.is_closed());
}

#[tokio::test]
async fn local_heartbeat_survives_internal_drain_until_http_has_stopped() {
    let state = local_state().await;
    let local = state.local_runtime.as_ref().unwrap();
    let before: i64 = sqlx::query_scalar("SELECT heartbeat FROM runtime_instances WHERE id = ?")
        .bind(&local.owner)
        .fetch_one(&local.pool)
        .await
        .unwrap();
    crate::local_runtime::spawn_maintenance(&state);
    state.lifecycle.begin_drain("synthetic internal drain");
    state.shutdown.request("synthetic internal drain");
    // Exercise the actual ten-second interval, not merely a still-pending first poll.
    tokio::time::sleep(Duration::from_secs(11)).await;
    let after: i64 = sqlx::query_scalar("SELECT heartbeat FROM runtime_instances WHERE id = ?")
        .bind(&local.owner)
        .fetch_one(&local.pool)
        .await
        .unwrap();
    assert!(
        after > before,
        "internal drain stopped the actual heartbeat tick"
    );
    assert!(!state.lifecycle.local_maintenance_stop.is_requested());
    assert_eq!(
        state.lifecycle.background_tasks.workers(),
        [Worker::LocalMaintenance]
    );
    assert!(!state
        .lifecycle
        .background_tasks
        .state_for_test_finished(Worker::LocalMaintenance));
    state
        .lifecycle
        .local_maintenance_stop
        .request("synthetic HTTP stopped");
    state.lifecycle.background_tasks.join().await.unwrap();
    state.local_runtime.as_ref().unwrap().close().await;
}

fn enable_empty_automation(state: &mut Arc<AppState>) {
    let state = Arc::get_mut(state).unwrap();
    state.credential_pool_automation = Arc::new(
        crate::credential_pool_automation::CredentialPoolAutomationRuntime::load(
            &crate::credential_pool_automation::CredentialPoolAutomationConfig {
                enabled: true,
                refill_queue_enabled: true,
                ..Default::default()
            },
        )
        .unwrap(),
    );
}

#[tokio::test]
async fn already_requested_shutdown_stops_all_legacy_loops_before_the_first_sweep() {
    let mut state = local_state().await;
    enable_empty_automation(&mut state);
    let mutable = Arc::get_mut(&mut state).unwrap();
    mutable.config.provider_credential_refresh_enabled = true;
    mutable.config.credential_stock_monitor_enabled = true;
    mutable.pg_pool =
        Some(sqlx::PgPool::connect_lazy("postgres://fixture:fixture@127.0.0.1:1/fixture").unwrap());
    state.shutdown.request("before first sweep");
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::join!(
            crate::credential_refill::start_credential_refill_notification_task(state.clone()),
            Box::pin(crate::credential_pool_automation::start_credential_pool_automation_task(state.clone())),
            Box::pin(crate::provider_credential_probe_scheduler::start_provider_credential_probe_scheduler(
                state.clone()
            )),
            Box::pin(crate::provider_credential_refresh::start_provider_credential_refresh_task(
                state.clone()
            )),
            crate::credential_stock::start_credential_stock_monitor_task(state.clone()),
            crate::credential_store::start_credential_refresh_task(
                state.credential_cache.clone(),
                state.redis_pool.clone(),
                60,
                state.shutdown.clone()
            ),
        );
    })
    .await
    .unwrap();
    state.pg_pool.as_ref().unwrap().close().await;
    state.local_runtime.as_ref().unwrap().close().await;
}

#[tokio::test]
async fn idle_empty_schedulers_wake_on_shutdown_without_waiting_for_next_tick() {
    let mut state = local_state().await;
    enable_empty_automation(&mut state);
    let worker_state = state.clone();
    let task = tokio::spawn(async move {
        tokio::join!(
            crate::credential_refill::start_credential_refill_notification_task(
                worker_state.clone()
            ),
            Box::pin(crate::credential_pool_automation::start_credential_pool_automation_task(
                worker_state.clone()
            )),
            Box::pin(crate::provider_credential_probe_scheduler::start_provider_credential_probe_scheduler(
                worker_state
            )),
        );
    });
    tokio::time::sleep(Duration::from_millis(25)).await;
    state.shutdown.request("idle scheduler shutdown");
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    state.local_runtime.as_ref().unwrap().close().await;
}
