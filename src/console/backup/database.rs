//! Validate an untrusted uploaded SQLite snapshot read-only, without executing schema code.
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row,
};
use std::path::Path;

pub(super) async fn validate_archive(
    root: &Path,
    files: &std::collections::BTreeMap<String, Vec<u8>>,
) -> Result<crate::routing::config::RouteConfigYaml, String> {
    for (name, bytes) in files {
        let path = gateway_local_data::restore::checked(root, name)
            .map_err(|_| "Unsafe backup file path")?;
        std::fs::create_dir_all(path.parent().ok_or("Invalid backup file path")?)
            .map_err(|_| "Cannot stage backup validation")?;
        std::fs::write(path, bytes).map_err(|_| "Cannot stage backup validation")?;
    }
    validate(&root.join("local/state/runtime.sqlite3")).await?;
    let document: crate::routing::config::RouteConfigYaml =
        serde_yaml::from_slice(&files["local/routes.yaml"]).map_err(|_| "Invalid backup routes")?;
    let config = crate::console::ConsoleConfig::from_values(crate::console::ConsoleConfigValues {
        state_dir: Some(root.join("local/state")),
        routes_file: Some(root.join("local/routes.yaml")),
        ..Default::default()
    })
    .map_err(|_| "Invalid backup layout")?;
    let auth = crate::console::ConsoleAuthRuntime::new(&config, None)
        .map_err(|_| "Invalid administrator backup")?;
    if !auth
        .managed_keys_active()
        .map_err(|_| "Invalid administrator keys")?
    {
        return Err("Backup is missing administrator keys".into());
    }
    let runtime = crate::console::RouteConfigRuntime::new_local(&config)
        .map_err(|_| "Invalid route recovery archive")?;
    runtime
        .recover_startup()
        .await
        .map_err(|_| "Backup route recovery validation failed")?;
    if serde_json::to_value(runtime.route_config().snapshot().document())
        .map_err(|_| "Invalid backup routes")?
        != serde_json::to_value(&document).map_err(|_| "Invalid backup routes")?
    {
        return Err("Backup route document does not match its active revision".into());
    }
    if let Some(bytes) = files.get("gateway.json") {
        serde_json::from_slice::<gateway_local_data::startup::StartupConfig>(bytes)
            .map_err(|_| "Invalid gateway startup settings")?;
    }
    let local = crate::local_runtime::LocalRuntime::open(&root.join("local/state"))
        .await
        .map_err(|_| "Backup database cannot be opened by this Gateway version")?;
    local.close().await;
    Ok(document)
}

pub(super) async fn validate(path: &Path) -> Result<(), String> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .read_only(true)
                .pragma("trusted_schema", "OFF"),
        )
        .await
        .map_err(|_| "Invalid backup database")?;
    let result = async {
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&pool)
            .await
            .map_err(|_| "Invalid database version")?;
        if version != 1 {
            return Err("Unsupported database version".into());
        }
        let virtual_tables: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pragma_table_list WHERE type IN ('virtual','shadow')",
        )
        .fetch_one(&pool)
        .await
        .map_err(|_| "Cannot validate backup table types")?;
        if virtual_tables != 0 {
            return Err("Backup database contains unsupported virtual tables".into());
        }
        let objects =
            sqlx::query("SELECT type, name FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'")
                .fetch_all(&pool)
                .await
                .map_err(|_| "Invalid database schema")?;
        for row in objects {
            let kind: String = row.get("type");
            if kind != "table" && kind != "index" {
                return Err("Backup database contains unsupported executable schema".into());
            }
        }
        for table in [
            "local_access_keys",
            "local_access_key_secrets",
            "local_balance_accounts",
            "gateway_cash_accounts",
            "request_audits",
        ] {
            let present: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .map_err(|_| "Invalid database tables")?;
            if present != 1 {
                return Err("Backup database is missing required tables".into());
            }
        }
        let result: String = sqlx::query_scalar("PRAGMA quick_check")
            .fetch_one(&pool)
            .await
            .map_err(|_| "Backup database integrity check failed")?;
        if result != "ok" {
            return Err("Backup database integrity check failed".into());
        }
        Ok(())
    }
    .await;
    pool.close().await;
    result
}
