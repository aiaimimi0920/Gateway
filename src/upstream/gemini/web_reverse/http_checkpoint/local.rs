//! Separate additive SQLite owner: bounded records, durable commit and cross-process CAS.
use super::{error, Config, MAX_BYTES};
use crate::error::GatewayError;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
    Connection, Row, SqliteConnection,
};
use std::{path::PathBuf, time::Duration};

fn prepare(directory: PathBuf) -> Result<PathBuf, GatewayError> {
    let fail = || error("local_storage_failed");
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&directory).map_err(|_| fail())?;
    let metadata = std::fs::symlink_metadata(&directory).map_err(|_| fail())?;
    if !metadata.is_dir() || reparse(&metadata) {
        return Err(fail());
    }
    let file = directory.join("http-auth-checkpoints-v1.sqlite3");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&file) {
        Ok(_) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(_) => return Err(fail()),
    }
    let metadata = std::fs::symlink_metadata(&file).map_err(|_| fail())?;
    if !metadata.is_file() || reparse(&metadata) || metadata.len() > 256 * 1024 * 1024 {
        return Err(fail());
    }
    Ok(file)
}

fn reparse(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

pub(super) async fn open(config: &Config) -> Result<SqliteConnection, GatewayError> {
    let directory = config.directory.clone();
    let file = tokio::task::spawn_blocking(move || prepare(directory))
        .await
        .map_err(|_| error("local_storage_failed"))??;
    let options = SqliteConnectOptions::new()
        .filename(file)
        .create_if_missing(false)
        .journal_mode(SqliteJournalMode::Delete)
        .synchronous(SqliteSynchronous::Full)
        .busy_timeout(Duration::from_millis(250))
        .pragma("trusted_schema", "OFF")
        .pragma("max_page_count", "65536");
    let mut db = SqliteConnection::connect_with(&options)
        .await
        .map_err(|_| error("local_storage_failed"))?;
    sqlx::query("CREATE TABLE IF NOT EXISTS http_auth_checkpoint_v1 (binding TEXT PRIMARY KEY, revision TEXT NOT NULL, payload BLOB NOT NULL)")
        .execute(&mut db).await.map_err(|_| error("local_storage_failed"))?;
    Ok(db)
}

pub(super) async fn read(
    config: &Config,
    binding: &str,
) -> Result<Option<(String, Vec<u8>)>, GatewayError> {
    let mut db = open(config).await?;
    let row = sqlx::query(
        "SELECT revision, length(payload) AS size FROM http_auth_checkpoint_v1 WHERE binding=?",
    )
    .bind(binding)
    .fetch_optional(&mut db)
    .await
    .map_err(|_| error("local_storage_failed"))?;
    let Some(row) = row else {
        db.close()
            .await
            .map_err(|_| error("local_storage_failed"))?;
        return Ok(None);
    };
    let revision: String = row
        .try_get("revision")
        .map_err(|_| error("invalid_record"))?;
    let size: i64 = row.try_get("size").map_err(|_| error("invalid_record"))?;
    if !(34..=MAX_BYTES as i64).contains(&size) {
        return Err(error("too_large"));
    }
    let bytes: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT payload FROM http_auth_checkpoint_v1 WHERE binding=? AND revision=?",
    )
    .bind(binding)
    .bind(&revision)
    .fetch_optional(&mut db)
    .await
    .map_err(|_| error("local_storage_failed"))?;
    db.close()
        .await
        .map_err(|_| error("local_storage_failed"))?;
    bytes
        .map(|bytes| (revision, bytes))
        .map(Some)
        .ok_or_else(|| error("conflict"))
}

pub(super) async fn write(
    config: &Config,
    binding: &str,
    token: Option<&str>,
    revision: &str,
    bytes: &[u8],
) -> Result<(), GatewayError> {
    if bytes.len() > MAX_BYTES {
        return Err(error("too_large"));
    }
    let mut db = open(config).await?;
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| error("local_storage_failed"))?;
    let count = if let Some(token) = token {
        sqlx::query("UPDATE http_auth_checkpoint_v1 SET revision=?,payload=? WHERE binding=? AND revision=?")
            .bind(revision).bind(bytes).bind(binding).bind(token).execute(&mut *tx).await
    } else {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM http_auth_checkpoint_v1").fetch_one(&mut *tx).await
            .map_err(|_| error("local_storage_failed"))?;
        if count >= 1024 { return Err(error("capacity")); }
        sqlx::query("INSERT OR IGNORE INTO http_auth_checkpoint_v1 (binding,revision,payload) VALUES (?,?,?)")
            .bind(binding).bind(revision).bind(bytes).execute(&mut *tx).await
    }.map_err(|_| error("local_storage_failed"))?.rows_affected();
    if count != 1 {
        return Err(error("conflict"));
    }
    tx.commit()
        .await
        .map_err(|_| error("local_storage_failed"))?;
    db.close().await.map_err(|_| error("local_storage_failed"))
}
