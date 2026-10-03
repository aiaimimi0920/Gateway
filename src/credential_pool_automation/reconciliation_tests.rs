//! Exercise cycle-only persistence through a real local route CAS and loopback driver.
use super::*;
use crate::config::{Config, GatewayRuntimeRole, GatewayStorageMode};
use crate::console::{ConsoleConfig, ConsoleConfigValues, RouteConfigRuntime};
use axum::{routing::post, Json, Router};

#[path = "reconciliation_cloud_archive_tests.rs"]
mod cloud_archive;
#[path = "purge_race_tests.rs"]
mod purge_races;

#[tokio::test]
async fn empty_delivery_persists_cycle_when_health_recovers_above_minimum() {
    let root = std::env::temp_dir().join(format!("gateway-local-reconcile-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let routes = root.join("routes.yaml");
    fs::write(&routes, serde_json::to_vec(&serde_json::json!({
        "providers": [{"id":"p", "base_url":"https://example.invalid", "auto_refill_enabled":true,
            "pool_min_size":1, "pool_target_size":3,
            "credentials":[{"id":"a","api_key":"fixture-a"},{"id":"b","api_key":"fixture-b"}]}],
        "model_routes":[], "aliases":{}
    })).unwrap()).unwrap();
    let console = ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(root.join("state")),
        routes_file: Some(routes),
        ..Default::default()
    })
    .unwrap();
    let state = crate::runtime::build_app_state(local_config(console.clone()))
        .await
        .unwrap();
    let local = state.local_runtime.as_ref().unwrap();
    for id in ["a", "b"] {
        let payload = serde_json::json!({"providerAccountId":"p", "providerCredentialRef":id,
            "status":"cooling", "failureScope":"credential_model"});
        sqlx::query("INSERT INTO credential_model_states(id, payload, updated) VALUES (?, ?, 0)")
            .bind(id)
            .bind(payload.to_string())
            .execute(&local.pool)
            .await
            .unwrap();
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/refill", listener.local_addr().unwrap());
    let pool = local.pool.clone();
    let router = Router::new().route("/refill", post(move || {
        let pool = pool.clone();
        async move {
            // Health can recover while a driver runs without changing the route revision.
            sqlx::query("UPDATE credential_model_states SET payload = json_set(payload, '$.status', 'active')")
                .execute(&pool).await.unwrap();
            Json(serde_json::json!({"credentials":[], "prune":[]}))
        }
    }));
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
    });
    let driver = CredentialAutomationDriver {
        id: "fixture-driver".into(),
        provider_ids: vec!["p".into()],
        timeout_secs: Some(5),
        transport: CredentialAutomationDriverTransport::Http {
            endpoint,
            secret_env: None,
        },
    };
    let result: anyhow::Result<()> = async {
        let snapshot = state.route_config.snapshot();
        let provider = snapshot.document().providers[0].clone();
        anyhow::ensure!(!provider.pool_refill_in_progress);
        let outcome = reconciliation::reconcile_provider_with_driver(
            &state,
            &snapshot,
            provider,
            &driver,
            CredentialPoolAutomationAction::Reconcile,
        )
        .await?;
        anyhow::ensure!(outcome.created_count == 0 && outcome.pruned_count == 0);
        let current = state.route_config.snapshot();
        anyhow::ensure!(
            current.revision().id() != snapshot.revision().id(),
            "cycle-only change was not committed"
        );
        anyhow::ensure!(current.document().providers[0].pool_refill_in_progress);
        Ok(())
    }
    .await;
    let _ = stop.send(());
    server.await.unwrap().unwrap();
    local.close().await;
    drop(state);
    let reloaded = RouteConfigRuntime::new_local(&console).unwrap();
    let persisted =
        reloaded.route_config().snapshot().document().providers[0].pool_refill_in_progress;
    drop(reloaded);
    crate::local_runtime::test_support::remove_test_root(&root).await;
    result.unwrap();
    assert!(
        persisted,
        "refill cycle must survive a local runtime restart"
    );
}

fn local_config(console: ConsoleConfig) -> Config {
    Config {
        console,
        runtime_role: GatewayRuntimeRole::Standalone,
        storage_mode: GatewayStorageMode::Local,
        port: 0,
        redis_url: "redis://127.0.0.1:1".into(),
        database_url: None,
        upstream_timeout_secs: 5,
        max_request_body_bytes: 1024 * 1024,
        max_body_chat_completions_bytes: 1024 * 1024,
        max_body_completions_bytes: 1024 * 1024,
        max_body_messages_bytes: 1024 * 1024,
        max_body_responses_bytes: 1024 * 1024,
        max_body_embeddings_bytes: 1024 * 1024,
        max_body_audio_transcriptions_bytes: 1024 * 1024,
        max_body_audio_speech_bytes: 1024 * 1024,
        max_body_search_bytes: 1024 * 1024,
        max_body_fetch_bytes: 1024 * 1024,
        max_body_research_bytes: 1024 * 1024,
        max_body_images_generations_bytes: 1024 * 1024,
        max_body_images_edits_bytes: 1024 * 1024,
        max_body_music_bytes: 1024 * 1024,
        max_body_videos_bytes: 1024 * 1024,
        response_cache_ttl_secs: 300,
        response_cache_max_size_bytes: 1024 * 1024,
        quota_pre_deduct_estimate_ratio: 1.2,
        usage_report_batch_size: 100,
        provider_probe_interval_secs: 30,
        log_level: "info".into(),
        gateway_api_key: None,
        gateway_api_key_secret: None,
        gateway_management_token: Some("fixture-management-token".into()),
        gateway_keepalive_bearer_token: None,
        default_project_id: "fixture".into(),
        gateway_inbound_api_key_header_aliases: vec![],
        provider_credential_folder_sync_enabled: false,
        provider_credential_folder_sync_root_dir: None,
        provider_credential_folder_sync_interval_secs: 30,
        provider_credential_folder_sync_import_enabled: false,
        provider_credential_folder_sync_export_enabled: false,
        provider_credential_folder_sync_watch_enabled: false,
        provider_credential_folder_sync_watch_debounce_millis: 1500,
        provider_credential_folder_sync_delete_missing: false,
        provider_credential_refresh_enabled: false,
        provider_credential_refresh_interval_secs: 3600,
        provider_credential_refresh_before_secs: 86400,
        provider_credential_refresh_batch_limit: 100,
        provider_credential_refresh_lock_ttl_secs: 300,
        credential_stock_monitor_enabled: false,
        credential_stock_monitor_interval_secs: 60,
        credential_pool_automation: Default::default(),
        splitter_worker_executable_path: None,
        splitter_initial_worker_port: 4201,
        splitter_ready_timeout_secs: 120,
        splitter_ready_poll_interval_millis: 500,
        splitter_reload_shutdown_timeout_secs: 600,
    }
}
