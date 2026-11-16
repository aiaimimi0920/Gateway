//! Isolated full data round trip; no live accounts or remote cloud destinations are used.
use neuro_gateway::{
    console::{backup::service, ConsoleAuthRuntime, RouteConfigRuntime},
    local_runtime::LocalRuntime,
};
use std::{fs, path::PathBuf, sync::Arc};
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

async fn fixture() -> (ConsoleStateFixture, PathBuf, fs::File) {
    let mut fixture = ConsoleStateFixture::new(
        document("backup-provider", "model", "synthetic-provider-secret"),
        false,
    );
    let root = std::env::temp_dir().join(format!("gateway-backup-{}", uuid::Uuid::new_v4()));
    std::env::set_var("AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR", root.join("objects"));
    let lease = gateway_local_data::prepare(&root).unwrap();
    fs::create_dir_all(root.join("local/state")).unwrap();
    let state = Arc::get_mut(&mut fixture.state).unwrap();
    state.config.console.state_dir = root.join("local/state");
    state.config.console.routes_file = root.join("local/routes.yaml");
    fs::write(
        &state.config.console.routes_file,
        serde_yaml::to_string(&document(
            "backup-provider",
            "model",
            "synthetic-provider-secret",
        ))
        .unwrap(),
    )
    .unwrap();
    let runtime = RouteConfigRuntime::new_local(&state.config.console).unwrap();
    state.route_config = Arc::clone(runtime.route_config());
    state.route_config_runtime = Some(Arc::new(runtime));
    state.console_auth = Arc::new(
        ConsoleAuthRuntime::new(&state.config.console, Some(MANAGEMENT_TOKEN.into())).unwrap(),
    );
    state.local_runtime = Some(
        LocalRuntime::open(&state.config.console.state_dir)
            .await
            .unwrap(),
    );
    (fixture, root, lease)
}

#[test]
fn complete_backup_preserves_key_seal_balance_material_and_requires_restart() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (source_root, target_root) = runtime.block_on(async {
        std::env::remove_var("AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR");
        std::env::set_var("AI_GATEWAY_OBJECT_STORAGE_DRIVER", "local");
        let (source, source_root, source_lease) = fixture().await;
        let endpoint = "/v1/internal/gateway/console/backup";
        let denied = send_console_json(
            &source.state,
            axum::http::Method::POST,
            endpoint,
            serde_json::json!({"action":"status"}),
            None,
            None,
        )
        .await;
        assert!(!denied.status().is_success());
        let grant = grant_console_secret_access(&source.state).await;
        let status = send_console_json(
            &source.state,
            axum::http::Method::POST,
            endpoint,
            serde_json::json!({"action":"status"}),
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(status.status(), axum::http::StatusCode::OK);
        assert_eq!(status.headers()["cache-control"], "no-store");
        assert_eq!(parse_json(status).await["pending"], false);
        let local = source.state.local_runtime.as_ref().unwrap();
        let key = local
            .save_access_key_with_quota(
                None,
                neuro_gateway::db::UpsertAccessKeyInput {
                    owner_type: "user".into(),
                    owner_id: "fixture-owner".into(),
                    resolved_project_id: "local".into(),
                    resolved_tenant_id: "local".into(),
                    key_kind: "user".into(),
                    public_key_prefix: "sk-gw".into(),
                    display_name: "Backup test".into(),
                    expires_at: None,
                    bundle_ids: vec![],
                    metadata: Some(
                        serde_json::json!({"scopes":["chat.completions"], "models":["model"]}),
                    ),
                },
                Some(&neuro_gateway::access_balance::quota::KeyQuotaInput {
                    mode: "message_prepaid".into(),
                    limit: Some(73),
                    currency: None,
                }),
            )
            .await
            .unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(source_root.join("local/state/runtime.sqlite3")),
            )
            .await
            .unwrap();
        let original_balance: String =
            sqlx::query_scalar("SELECT payload FROM local_balance_accounts LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO request_audits VALUES ('old', '{}', '2026-01-01', 'completed', 'fixture')",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
        fs::create_dir_all(source_root.join("objects/credential-runtime/fixture")).unwrap();
        fs::write(
            source_root.join("objects/credential-runtime/fixture/session.json"),
            b"synthetic-session",
        )
        .unwrap();
        let material = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(source_root.join("objects/session.sqlite3"))
                    .create_if_missing(true)
                    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal),
            )
            .await
            .unwrap();
        sqlx::query("CREATE TABLE session (value TEXT NOT NULL)")
            .execute(&material)
            .await
            .unwrap();
        sqlx::query("INSERT INTO session VALUES ('synthetic-wal-session')")
            .execute(&material)
            .await
            .unwrap();
        service::save_destination(
            &source.state,
            neuro_gateway::console::backup::remote::Destination::Webdav {
                url: "https://example.com/gateway.ngbk".into(),
                username: "fixture".into(),
                password: "synthetic-sync-password".into(),
            },
        )
        .unwrap();
        let response = send_console_json(
            &source.state,
            axum::http::Method::POST,
            endpoint,
            serde_json::json!({"action":"export", "password":"fixture-backup-password"}),
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = parse_json(response).await;
        let encrypted = service::decode_file(body["content"].as_str().unwrap()).unwrap();
        material.close().await;
        assert!(!encrypted
            .windows(b"synthetic-sync-password".len())
            .any(|part| part == b"synthetic-sync-password"));
        let (target, target_root, target_lease) = fixture().await;
        assert!(service::preview(
            &target.state,
            encrypted.clone(),
            "incorrect-password".into()
        )
        .await
        .is_err());
        assert!(!gateway_local_data::restore::pending(&target_root).unwrap());
        assert!(
            service::preview(
                &target.state,
                encrypted.clone(),
                "fixture-backup-password".into()
            )
            .await
            .unwrap()
            .replaces_access_keys_and_balances
        );
        let revision = target
            .state
            .route_config
            .snapshot()
            .revision()
            .id()
            .to_string();
        service::restore(
            &target.state,
            encrypted,
            "fixture-backup-password".into(),
            &revision,
        )
        .await
        .unwrap();
        assert!(gateway_local_data::restore::pending(&target_root).unwrap());
        assert!(gateway_local_data::prepare(&target_root).is_err());
        assert!(target
            .state
            .local_runtime
            .as_ref()
            .unwrap()
            .access_catalog()
            .await
            .unwrap()
            .access_keys
            .is_empty());
        target.state.local_runtime.as_ref().unwrap().close().await;
        drop(target_lease);
        drop(target);
        let restored_lease = gateway_local_data::prepare(&target_root).unwrap();
        let restored = LocalRuntime::open(&target_root.join("local/state"))
            .await
            .unwrap();
        let restored_config = neuro_gateway::console::ConsoleConfig::from_values(
            neuro_gateway::console::ConsoleConfigValues {
                state_dir: Some(target_root.join("local/state")),
                routes_file: Some(target_root.join("local/routes.yaml")),
                ..Default::default()
            },
        )
        .unwrap();
        let restored_auth =
            ConsoleAuthRuntime::new(&restored_config, Some("obsolete-target-token".into()))
                .unwrap();
        let context = neuro_gateway::console::ConsoleRequestContext::loopback();
        assert!(restored_auth
            .authenticate_management_token(&context, MANAGEMENT_TOKEN)
            .is_ok());
        assert!(restored_auth
            .authenticate_management_token(&context, "obsolete-target-token")
            .is_err());
        let request = neuro_gateway::auth::adapter::AuthRequest {
            authorization: Some(format!("Bearer {}", key.token.unwrap())),
            api_key: None,
            path: "/v1/chat/completions".into(),
            method: "POST".into(),
        };
        assert!(restored
            .authenticate_access_key(&request)
            .await
            .unwrap()
            .is_some());
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(target_root.join("local/state/runtime.sqlite3")),
            )
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT payload FROM local_balance_accounts LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap(),
            original_balance
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM request_audits WHERE id='old'")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        pool.close().await;
        assert_eq!(
            fs::read(target_root.join("objects/credential-runtime/fixture/session.json")).unwrap(),
            b"synthetic-session"
        );
        assert!(target_root.join("backups").is_dir());
        let mut restored_fixture = ConsoleStateFixture::new(
            document("backup-provider", "model", "synthetic-provider-secret"),
            false,
        );
        let state = Arc::get_mut(&mut restored_fixture.state).unwrap();
        state.config.console = restored_config;
        state.local_runtime = Some(restored.clone());
        match service::destination(state).unwrap().unwrap() {
            neuro_gateway::console::backup::remote::Destination::Webdav { password, .. } => {
                assert_eq!(password, "synthetic-sync-password");
            }
            _ => panic!("Restored destination changed kind"),
        }
        drop(restored_fixture);
        let material = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(target_root.join("objects/session.sqlite3"))
                    .read_only(true),
            )
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT value FROM session")
                .fetch_one(&material)
                .await
                .unwrap(),
            "synthetic-wal-session"
        );
        material.close().await;
        restored.close().await;
        drop(restored_lease);
        local.close().await;
        drop(source_lease);
        drop(source);
        (source_root, target_root)
    });
    // HTTP routing owns background tasks; stop that runtime before removing its file fixtures.
    drop(runtime);
    for root in [source_root, target_root] {
        // SQLite closes its worker-thread file handles just after pool shutdown on Windows.
        for attempt in 0..40 {
            match fs::remove_dir_all(&root) {
                Ok(()) => break,
                Err(error) if cfg!(windows) && error.raw_os_error() == Some(32) && attempt < 39 => {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(error) => panic!("fixture cleanup failed: {error}"),
            }
        }
    }
}
