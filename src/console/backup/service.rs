//! Backend-owned manual backup workflow. HTTP admission supplies management and secret grants.
use super::{
    archive::Archive,
    database, files,
    remote::{self, Destination},
};
use crate::{console::RouteConfigPersistence, state::AppState};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures::FutureExt;
use gateway_local_data::restore::{checked, MAX_BYTES};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
static CRYPTO: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub created_at: String,
    pub files: usize,
    pub providers: usize,
    pub accounts: usize,
    pub replaces_access_keys_and_balances: bool,
}

pub fn root(state: &AppState) -> Result<PathBuf, String> {
    if state.local_runtime.is_none() {
        return Err("Full backup currently requires local SQLite storage".into());
    }
    let root = state
        .config
        .console
        .state_dir
        .parent()
        .and_then(Path::parent)
        .ok_or("Unsupported data layout")?
        .to_owned();
    if state.config.console.state_dir != root.join("local/state")
        || state.config.console.routes_file != root.join("local/routes.yaml")
    {
        return Err("Backup requires the managed Gateway data layout".into());
    }
    let object_driver = ["AI_GATEWAY_OBJECT_STORAGE_DRIVER", "OBJECT_STORAGE_DRIVER"]
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "local".into());
    let object_root = [
        "AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR",
        "CREDENTIAL_OBJECT_STORAGE_LOCAL_DIR",
        "OBJECT_STORAGE_LOCAL_DIR",
    ]
    .iter()
    .find_map(|name| std::env::var(name).ok())
    .map(|value| value.trim().to_owned())
    .filter(|value| !value.is_empty())
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from(".runtime/ai-gateway-objects"));
    if object_driver == "s3-compatible" || object_root != root.join("objects") {
        return Err("Full backup of external object storage is not supported; no partial backup was created".into());
    }
    checked(&root, "local").map_err(|e| e.to_string())?;
    Ok(root)
}

fn persistence(state: &AppState) -> Result<RouteConfigPersistence, String> {
    RouteConfigPersistence::new(
        &state.config.console.state_dir,
        &state.config.console.routes_file,
    )
    .map_err(|_| "Cannot access backup configuration".into())
}

pub async fn export(state: &AppState, password: String) -> Result<Vec<u8>, String> {
    let permit = CRYPTO
        .try_acquire()
        .map_err(|_| "Backup operation is busy")?;
    let root = root(state)?;
    let persistence = persistence(state)?;
    let _publication = state.lifecycle.health_control.publication.lock().await;
    let _writer = persistence
        .try_writer_lock()
        .map_err(|_| "Configuration is busy; retry backup when idle")?;
    let temporary = Temporary::new(&root)?;
    let database = temporary.0.join("runtime.sqlite3");
    state
        .local_runtime
        .as_ref()
        .ok_or("Local storage unavailable")?
        .backup_database(&database)
        .boxed()
        .await?;
    let collect_root = root.clone();
    let database_bytes = read(&database, MAX_BYTES)?;
    let (mut files, permit) = tokio::task::spawn_blocking(move || {
        let temporary = temporary;
        files::collect(&collect_root, &temporary.0).map(|files| (files, permit))
    })
    .await
    .map_err(|_| "Backup collection task failed")?
    .map_err(|_| {
        "Cannot collect complete backup; data may be busy, linked or exceed the size limit"
    })?;
    files.insert("local/state/runtime.sqlite3".into(), database_bytes);
    files.insert(
        "local/routes.yaml".into(),
        serde_yaml::to_string(state.route_config.snapshot().document())
            .map_err(|_| "Cannot encode active routes")?
            .into_bytes(),
    );
    files.insert(
        "local/state/console/admin.json".into(),
        state
            .console_auth
            .backup_key_ring()
            .map_err(|_| "Cannot export administrator keys")?,
    );
    // Authenticate the complete payload before returning any bytes to HTTP or a remote destination.
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut payload = Archive::encode(files)?;
        let result = super::encrypt(&payload, &password).map_err(str::to_owned);
        payload.fill(0);
        result
    })
    .await
    .map_err(|_| "Backup encryption task failed")?
}

async fn decode(
    state: &AppState,
    bytes: Vec<u8>,
    password: String,
) -> Result<(Preview, BTreeMap<String, Vec<u8>>), String> {
    let root = root(state)?;
    let permit = CRYPTO
        .try_acquire()
        .map_err(|_| "Backup decryption is busy")?;
    let runtime = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut payload = super::decrypt(&bytes, &password).map_err(str::to_owned)?;
        let result = Archive::decode(&payload);
        payload.fill(0);
        let (created_at, files) = result?;
        let temporary = Temporary::new(&root)?;
        // Own staging through cancellation; startup validation need not be a Send future.
        let document = runtime.block_on(database::validate_archive(&temporary.0, &files))?;
        let preview = Preview {
            created_at,
            files: files.len(),
            providers: document.providers.len(),
            accounts: document
                .providers
                .iter()
                .map(|provider| provider.credentials.len())
                .sum(),
            replaces_access_keys_and_balances: true,
        };
        Ok((preview, files))
    })
    .await
    .map_err(|_| "Backup validation task failed")?
}

pub async fn preview(
    state: &AppState,
    bytes: Vec<u8>,
    password: String,
) -> Result<Preview, String> {
    decode(state, bytes, password)
        .await
        .map(|(preview, _)| preview)
}

pub async fn restore(
    state: &AppState,
    bytes: Vec<u8>,
    password: String,
    expected_revision: &str,
) -> Result<(), String> {
    let root = root(state)?;
    let (_, files) = decode(state, bytes, password).await?;
    let _publication = state.lifecycle.health_control.publication.lock().await;
    let persistence = persistence(state)?;
    let _writer = persistence
        .try_writer_lock()
        .map_err(|_| "Configuration is busy")?;
    if state.route_config.snapshot().revision().id() != expected_revision {
        return Err("Gateway configuration changed; preview the backup again".into());
    }
    tokio::task::spawn_blocking(move || gateway_local_data::restore::stage(&root, &files))
        .await
        .map_err(|_| "Restore staging task failed")?
        .map_err(|error| error.to_string())
}

pub fn destination(state: &AppState) -> Result<Option<Destination>, String> {
    let path = checked(&root(state)?, "local/state/console/backup-sync.json")
        .map_err(|e| e.to_string())?;
    if !path.exists() {
        return Ok(None);
    }
    let sealed: String = serde_json::from_slice(&read(&path, 64 * 1024)?)
        .map_err(|_| "Invalid saved sync settings")?;
    let mut bytes = state
        .local_runtime
        .as_ref()
        .ok_or("Local storage unavailable")?
        .open_backup_destination(&sealed)?;
    let result =
        serde_json::from_slice::<Destination>(&bytes).map_err(|_| "Invalid saved sync settings");
    bytes.fill(0);
    let destination = result?;
    destination.validate()?;
    Ok(Some(destination))
}

pub fn save_destination(state: &AppState, mut destination: Destination) -> Result<(), String> {
    if let Destination::S3 { session_token, .. } = &mut destination {
        if session_token.as_ref().is_some_and(|value| value.is_empty()) {
            *session_token = None;
        }
    }
    root(state)?;
    destination.validate()?;
    let mut bytes = serde_json::to_vec(&destination).map_err(|_| "Cannot encode sync settings")?;
    let sealed = state
        .local_runtime
        .as_ref()
        .ok_or("Local storage unavailable")?
        .seal_backup_destination(&bytes);
    bytes.fill(0);
    let sealed = sealed?;
    let persistence = persistence(state)?;
    let guard = persistence
        .try_writer_lock()
        .map_err(|_| "Configuration is busy")?;
    // Store via the existing atomic writer, under the same data root and seal as the SQLite snapshot.
    persistence
        .atomic_write_json_locked(
            &guard,
            &persistence.state_root().join("backup-sync.json"),
            &sealed,
        )
        .map_err(|_| "Cannot save sync settings".into())
}

pub async fn upload(
    state: &AppState,
    password: String,
    expected: Option<&str>,
) -> Result<(), String> {
    let destination = destination(state)?.ok_or("Configure a sync destination first")?;
    let bytes = export(state, password).await?;
    remote::upload(&destination, bytes, expected).await
}
pub async fn download(state: &AppState) -> Result<remote::Download, String> {
    remote::download(&destination(state)?.ok_or("Configure a sync destination first")?).await
}
pub fn encode(bytes: Vec<u8>) -> String {
    STANDARD.encode(bytes)
}
pub fn decode_file(value: &str) -> Result<Vec<u8>, String> {
    if value.len() > 45 * 1024 * 1024 {
        return Err("Backup exceeds file size limit".into());
    }
    STANDARD
        .decode(value)
        .map_err(|_| "Invalid encrypted backup file".into())
}
fn read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| "Cannot read backup data")?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read backup data")?;
    if bytes.len() > limit {
        return Err("Backup file exceeds size limit".into());
    }
    Ok(bytes)
}
struct Temporary(PathBuf);
impl Temporary {
    fn new(root: &Path) -> Result<Self, String> {
        let path = checked(root, &format!("backups/.backup-{}", uuid::Uuid::new_v4()))
            .map_err(|e| e.to_string())?;
        gateway_local_data::restore::create_private_directory(&path)
            .map_err(|_| "Cannot create private backup workspace")?;
        Ok(Self(path))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            tracing::warn!(kind = ?error.kind(), "Could not remove temporary backup workspace");
        }
    }
}
