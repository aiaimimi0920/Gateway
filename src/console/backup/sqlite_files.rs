//! SQLite login materials need their own online snapshot, never a main/WAL file pair copy.
use futures::FutureExt;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{
    fs,
    io::{self, Read},
    path::Path,
};

pub(super) fn is_database(path: &Path) -> io::Result<bool> {
    let mut header = [0; 16];
    let mut file = fs::File::open(path)?;
    match file.read_exact(&mut header) {
        Ok(()) => Ok(&header == b"SQLite format 3\0"),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn is_sidecar(path: &Path) -> io::Result<bool> {
    let name = path.to_string_lossy();
    for suffix in ["-wal", "-shm", "-journal"] {
        if let Some(base) = name.strip_suffix(suffix) {
            let base = Path::new(base);
            if base.is_file() {
                return is_database(base);
            }
        }
    }
    Ok(false)
}

// Called only in the bounded blocking collector. The source is read-only; SQLite owns locking.
pub(super) fn snapshot(source: &Path, scratch: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let target = scratch.join(format!("material-{}.sqlite3", uuid::Uuid::new_v4()));
    tokio::runtime::Handle::current().block_on(async {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(source)
                    .read_only(true)
                    .pragma("trusted_schema", "OFF")
                    .busy_timeout(std::time::Duration::from_secs(3)),
            )
            .await
            .map_err(io::Error::other)?;
        let result = sqlx::query("VACUUM INTO ?")
            .bind(target.to_string_lossy().as_ref())
            .execute(&pool)
            .boxed()
            .await;
        pool.close().await;
        result.map_err(io::Error::other)?;
        Ok::<(), io::Error>(())
    })?;
    let mut bytes = Vec::new();
    fs::File::open(target)?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::other("Backup data exceeds 32 MiB"));
    }
    Ok(bytes)
}
