use std::env;
use std::error::Error;
#[cfg(windows)]
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub mod document;
pub mod journal;
pub mod persistence;
pub mod redis_store;
pub mod revision;
pub mod runtime;
pub mod secrets;

pub use journal::{
    JournalEntry, JournalError, RecoveryDisposition, RecoveryReport, TransactionJournal,
};
pub use persistence::{
    AtomicReplaceBackend, ConsolePersistenceError, FirstSavePresence, FirstSaveStatus,
    PersistenceError, PlatformAtomicReplaceBackend, RevisionArchive, RouteConfigPersistence,
    StoredRouteRevision, TransactionPhase, TransactionRecord, WriterLockGuard, YamlReplaceReceipt,
};
pub use redis_store::{
    RouteConfigRedisActivationOutcome, RouteConfigRedisKeys, RouteConfigRedisRevision,
    RouteConfigRedisStore, RouteConfigRedisStoreError,
};
pub use runtime::{
    PooledRouteConfigRedisBackend, RouteConfigCoordinator, RouteConfigRedisBackend,
    RouteConfigReplica, RouteConfigRuntime, RouteConfigRuntimeError,
};

const DEFAULT_REDIS_NAMESPACE: &str = "default";
const DEFAULT_SECRET_GRANT_TTL_SECS: u64 = 300;
const DEFAULT_MAX_EVENT_RECORDS: usize = 2_000;
const DEFAULT_STATE_DIRECTORY_NAME: &str = ".gateway-state";
const MAX_REDIS_NAMESPACE_BYTES: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsoleConfig {
    pub state_dir: PathBuf,
    pub routes_file: PathBuf,
    pub redis_namespace: String,
    pub remote_access_enabled: bool,
    pub trusted_origins: Vec<String>,
    pub tauri_origins: Vec<String>,
    pub secret_grant_ttl_secs: u64,
    pub max_event_records: usize,
    pub log_source: Option<PathBuf>,
    pub release_payload_root: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConsoleConfigValues {
    pub state_dir: Option<PathBuf>,
    pub routes_file: Option<PathBuf>,
    pub redis_namespace: Option<String>,
    pub remote_access_enabled: Option<bool>,
    pub trusted_origins: Option<Vec<String>>,
    pub tauri_origins: Option<Vec<String>>,
    pub secret_grant_ttl_secs: Option<u64>,
    pub max_event_records: Option<usize>,
    pub log_source: Option<PathBuf>,
    pub release_payload_root: Option<PathBuf>,
}

impl ConsoleConfigValues {
    pub fn from_env() -> Result<Self, ConsoleConfigError> {
        Ok(Self {
            state_dir: env_path("GATEWAY_STATE_DIR"),
            routes_file: env_path("GATEWAY_ROUTES_FILE"),
            redis_namespace: env_string("GATEWAY_CONSOLE_REDIS_NAMESPACE"),
            remote_access_enabled: parse_optional_env("GATEWAY_CONSOLE_REMOTE_ACCESS")?,
            trusted_origins: env_csv("GATEWAY_CONSOLE_TRUSTED_ORIGINS"),
            tauri_origins: env_csv("GATEWAY_CONSOLE_TAURI_ORIGINS"),
            secret_grant_ttl_secs: parse_optional_env("GATEWAY_CONSOLE_SECRET_GRANT_TTL_SECS")?,
            max_event_records: parse_optional_env("GATEWAY_CONSOLE_MAX_EVENTS")?,
            log_source: env_path("GATEWAY_CONSOLE_LOG_FILE"),
            release_payload_root: env_path("GATEWAY_RELEASE_PAYLOAD_ROOT"),
        })
    }
}

impl ConsoleConfig {
    pub fn from_env() -> Result<Self, ConsoleConfigError> {
        Self::from_values(ConsoleConfigValues::from_env()?)
    }

    pub fn from_values(values: ConsoleConfigValues) -> Result<Self, ConsoleConfigError> {
        let routes_file = values
            .routes_file
            .unwrap_or_else(|| PathBuf::from("routes.yaml"));
        let state_dir = values
            .state_dir
            .unwrap_or_else(|| default_state_directory(&routes_file));
        let release_payload_root = values.release_payload_root;
        let redis_namespace = non_empty_or_default(values.redis_namespace, DEFAULT_REDIS_NAMESPACE);
        validate_redis_namespace(&redis_namespace)?;

        let state_paths = resolve_path_views(&state_dir)?;
        let routes_paths = resolve_path_views(&routes_file)?;
        let release_paths = release_payload_root
            .as_deref()
            .map(resolve_path_views)
            .transpose()?;
        if let (Some(release_paths), Some(release_payload_root)) =
            (release_paths.as_ref(), release_payload_root.as_deref())
        {
            validate_state_path_views(
                &state_paths,
                release_paths,
                &state_dir,
                release_payload_root,
            )?;
            validate_routes_path_views(
                &routes_paths,
                release_paths,
                &routes_file,
                release_payload_root,
            )?;
        }

        Ok(Self {
            state_dir: state_paths.lexical,
            routes_file: routes_paths.lexical,
            redis_namespace,
            remote_access_enabled: values.remote_access_enabled.unwrap_or(false),
            trusted_origins: normalize_origins(values.trusted_origins.unwrap_or_default()),
            tauri_origins: normalize_origins(values.tauri_origins.unwrap_or_default()),
            secret_grant_ttl_secs: values
                .secret_grant_ttl_secs
                .unwrap_or(DEFAULT_SECRET_GRANT_TTL_SECS),
            max_event_records: values
                .max_event_records
                .unwrap_or(DEFAULT_MAX_EVENT_RECORDS),
            log_source: values.log_source,
            release_payload_root: release_paths.map(|paths| paths.lexical),
        })
    }
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self::from_values(ConsoleConfigValues::default())
            .expect("default console configuration must be valid")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsoleConfigError {
    code: &'static str,
    message: String,
}

impl ConsoleConfigError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for ConsoleConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ConsoleConfigError {}

pub fn validate_state_directory(
    state_dir: &Path,
    release_payload_root: Option<&Path>,
) -> Result<(), ConsoleConfigError> {
    let Some(release_payload_root) = release_payload_root else {
        return Ok(());
    };

    let state_paths = resolve_path_views(state_dir)?;
    let release_paths = resolve_path_views(release_payload_root)?;
    validate_state_path_views(
        &state_paths,
        &release_paths,
        state_dir,
        release_payload_root,
    )
}

pub(crate) fn validate_redis_namespace(namespace: &str) -> Result<(), ConsoleConfigError> {
    if namespace.is_empty()
        || namespace.len() > MAX_REDIS_NAMESPACE_BYTES
        || !namespace
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ConsoleConfigError::new(
            "console_invalid_redis_namespace",
            "Gateway console Redis namespace must be 1-64 ASCII letters, digits, '-' or '_'",
        ));
    }
    Ok(())
}

fn validate_state_path_views(
    state: &ResolvedPathViews,
    release: &ResolvedPathViews,
    state_dir: &Path,
    release_payload_root: &Path,
) -> Result<(), ConsoleConfigError> {
    if path_views_are_same_or_descendant(state, release) {
        return Err(ConsoleConfigError::new(
            "console_state_inside_release_payload",
            format!(
                "Gateway console state directory '{}' must be outside release payload '{}'",
                state_dir.display(),
                release_payload_root.display()
            ),
        ));
    }
    if path_views_are_same_or_descendant(release, state) {
        return Err(ConsoleConfigError::new(
            "console_state_contains_release_payload",
            format!(
                "Gateway console state directory '{}' must not contain release payload '{}'",
                state_dir.display(),
                release_payload_root.display()
            ),
        ));
    }

    Ok(())
}

fn validate_routes_path_views(
    routes: &ResolvedPathViews,
    release: &ResolvedPathViews,
    routes_file: &Path,
    release_payload_root: &Path,
) -> Result<(), ConsoleConfigError> {
    if path_views_are_same_or_descendant(routes, release) {
        return Err(ConsoleConfigError::new(
            "console_routes_inside_release_payload",
            format!(
                "Gateway routes file '{}' must be outside release payload '{}'",
                routes_file.display(),
                release_payload_root.display()
            ),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod persistence_lock_contract {
    use std::fs;
    use std::io;
    use std::path::Path;
    use std::sync::Arc;

    use time::OffsetDateTime;
    use uuid::Uuid;

    use super::document::canonicalize_route_document;
    use super::journal::{JournalAppendBackend, TransactionJournal};
    use super::persistence::{RouteConfigPersistence, TransactionRecord};
    use super::revision::{RevisionActor, RevisionMetadata};
    use crate::routing::config::RouteConfigYaml;

    #[test]
    fn one_writer_guard_spans_prepared_replace_and_phase_persistence() {
        let root =
            std::env::temp_dir().join(format!("gateway-console-locked-api-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let routes = root.join("routes.yaml");
        fs::write(&routes, b"providers: []\n").unwrap();
        let persistence = RouteConfigPersistence::for_test(&root, &routes).unwrap();
        let journal = TransactionJournal::new(persistence.clone());
        let document: RouteConfigYaml = serde_yaml::from_str(
            "providers: []\nmodel_routes: []\naliases:\n  managed: managed-model\n",
        )
        .unwrap();
        let canonical = canonicalize_route_document(&document).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let metadata = RevisionMetadata::from_canonical(
            1,
            Some("r0-000000000000".to_string()),
            RevisionActor::ManagementToken,
            now,
            None,
            &canonical,
        );
        let old_digest = {
            use sha2::{Digest, Sha256};
            hex::encode(Sha256::digest(b"providers: []\n"))
        };
        let record = TransactionRecord::new_prepared(
            "tx-one-guard",
            Some("r0-000000000000"),
            metadata.id(),
            format!("revisions/{}", metadata.id()),
            true,
            Some(&old_digest),
            canonical.yaml_digest(),
            now,
        )
        .unwrap();

        let guard = persistence.try_writer_lock().unwrap();
        persistence
            .archive_revision_locked(&guard, &metadata, &canonical)
            .unwrap();
        journal.persist_locked(&guard, &record).unwrap();
        let receipt = persistence
            .replace_routes_yaml_for_transaction_locked(&guard, &record, canonical.canonical_yaml())
            .unwrap();
        journal
            .record_yaml_replaced_locked(&guard, record.tx_id(), &receipt, now)
            .unwrap();

        assert_eq!(fs::read(&routes).unwrap(), canonical.canonical_yaml());
        assert_eq!(
            persistence.try_writer_lock().unwrap_err().code(),
            "console_locked"
        );
        drop(guard);
        let _ = fs::remove_dir_all(root);
    }

    #[derive(Debug)]
    struct FailingJournalAppend;

    impl JournalAppendBackend for FailingJournalAppend {
        fn append_and_sync(&self, _path: &Path, _bytes: &[u8]) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected journal failure",
            ))
        }
    }

    #[test]
    fn journal_append_failure_after_transaction_json_forces_read_only_recovery() {
        let root = std::env::temp_dir().join(format!(
            "gateway-console-journal-failure-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let routes = root.join("routes.yaml");
        let persistence = RouteConfigPersistence::for_test(&root, &routes).unwrap();
        let journal = TransactionJournal::with_append_backend_for_test(
            persistence.clone(),
            Arc::new(FailingJournalAppend),
        );
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let record = TransactionRecord::new_prepared(
            "tx-journal-failure",
            Some("r0-000000000000"),
            "r1-111111111111",
            "revisions/r1-111111111111",
            false,
            None,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            now,
        )
        .unwrap();

        let error = journal.persist(&record).unwrap_err();

        assert_eq!(error.code(), "console_recovery_required");
        assert!(persistence.is_read_only());
        assert!(persistence
            .transaction_record_path(record.tx_id())
            .unwrap()
            .is_file());
        assert!(!persistence.journal_path().exists());
        let _ = fs::remove_dir_all(root);
    }
}

fn default_state_directory(routes_file: &Path) -> PathBuf {
    routes_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(DEFAULT_STATE_DIRECTORY_NAME)
}

fn env_string(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_path(key: &str) -> Option<PathBuf> {
    env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn env_csv(key: &str) -> Option<Vec<String>> {
    env_string(key).map(|value| value.split(',').map(str::to_string).collect())
}

fn parse_optional_env<T>(key: &str) -> Result<Option<T>, ConsoleConfigError>
where
    T: std::str::FromStr,
    T::Err: fmt::Display,
{
    let Some(value) = env_string(key) else {
        return Ok(None);
    };

    value.parse::<T>().map(Some).map_err(|error| {
        ConsoleConfigError::new(
            "console_config_environment_invalid",
            format!("Invalid value for {key}: {error}"),
        )
    })
}

fn non_empty_or_default(value: Option<String>, default: &str) -> String {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn normalize_origins(origins: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for origin in origins {
        let origin = origin.trim();
        if !origin.is_empty() && !normalized.iter().any(|item| item == origin) {
            normalized.push(origin.to_string());
        }
    }
    normalized
}

#[derive(Debug)]
struct ResolvedPathViews {
    lexical: PathBuf,
    physical: PathBuf,
}

fn resolve_path_views(path: &Path) -> Result<ResolvedPathViews, ConsoleConfigError> {
    let absolute = std::path::absolute(path).map_err(|error| path_resolution_error(path, error))?;

    Ok(ResolvedPathViews {
        lexical: normalize_lexically(&absolute),
        physical: resolve_physical_path(&absolute)?,
    })
}

#[cfg(windows)]
fn resolve_physical_path(path: &Path) -> Result<PathBuf, ConsoleConfigError> {
    resolve_existing_ancestor(&normalize_lexically(path))
}

#[cfg(not(windows))]
fn resolve_physical_path(path: &Path) -> Result<PathBuf, ConsoleConfigError> {
    let mut resolved = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                resolved.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(value) => {
                let candidate = resolved.join(value);
                match fs::symlink_metadata(&candidate) {
                    Ok(_) => {
                        resolved = fs::canonicalize(&candidate)
                            .map_err(|error| path_resolution_error(path, error))?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        resolved.push(value);
                    }
                    Err(error) => return Err(path_resolution_error(path, error)),
                }
            }
        }
    }

    Ok(resolved)
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
        }
    }

    normalized
}

#[cfg(windows)]
fn resolve_existing_ancestor(path: &Path) -> Result<PathBuf, ConsoleConfigError> {
    let mut ancestor = path.to_path_buf();
    let mut missing_components = Vec::<OsString>::new();

    loop {
        match fs::symlink_metadata(&ancestor) {
            Ok(metadata) => {
                let mut resolved = canonicalize_existing_path(&ancestor, &metadata)
                    .map_err(|error| path_resolution_error(path, error))?;
                for component in missing_components.iter().rev() {
                    resolved.push(component);
                }
                return Ok(normalize_lexically(&resolved));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(component) = ancestor.file_name().map(OsString::from) else {
                    return Err(path_resolution_error(path, error));
                };
                missing_components.push(component);
                if !ancestor.pop() {
                    return Err(path_resolution_error(path, error));
                }
            }
            Err(error) => return Err(path_resolution_error(path, error)),
        }
    }
}

#[cfg(windows)]
fn canonicalize_existing_path(path: &Path, metadata: &fs::Metadata) -> std::io::Result<PathBuf> {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return fs::canonicalize(path);
    }

    fs::canonicalize(path)
}

fn path_resolution_error(path: &Path, error: std::io::Error) -> ConsoleConfigError {
    ConsoleConfigError::new(
        "console_path_resolution_failed",
        format!(
            "Failed to resolve Gateway console path '{}': {}",
            path.display(),
            error
        ),
    )
}

fn path_views_are_same_or_descendant(path: &ResolvedPathViews, parent: &ResolvedPathViews) -> bool {
    path_is_same_or_descendant(&path.lexical, &parent.lexical)
        || path_is_same_or_descendant(&path.physical, &parent.physical)
}

#[cfg(not(windows))]
fn path_is_same_or_descendant(path: &Path, parent: &Path) -> bool {
    path.starts_with(parent)
}

#[cfg(windows)]
fn path_is_same_or_descendant(path: &Path, parent: &Path) -> bool {
    let mut path_components = path
        .components()
        .filter(|component| !matches!(component, Component::CurDir));

    parent
        .components()
        .filter(|component| !matches!(component, Component::CurDir))
        .all(|parent_component| {
            path_components.next().is_some_and(|path_component| {
                windows_components_equal(path_component, parent_component)
            })
        })
}

#[cfg(windows)]
fn windows_components_equal(left: Component<'_>, right: Component<'_>) -> bool {
    match (left, right) {
        (Component::Prefix(left), Component::Prefix(right)) => {
            windows_prefixes_equal(left.kind(), right.kind())
        }
        (Component::RootDir, Component::RootDir)
        | (Component::CurDir, Component::CurDir)
        | (Component::ParentDir, Component::ParentDir) => true,
        (Component::Normal(left), Component::Normal(right)) => {
            windows_os_str_eq_ignore_case(left, right)
        }
        _ => false,
    }
}

#[cfg(windows)]
fn windows_prefixes_equal(left: std::path::Prefix<'_>, right: std::path::Prefix<'_>) -> bool {
    use std::path::Prefix;

    match (left, right) {
        (
            Prefix::Disk(left) | Prefix::VerbatimDisk(left),
            Prefix::Disk(right) | Prefix::VerbatimDisk(right),
        ) => left.eq_ignore_ascii_case(&right),
        (
            Prefix::UNC(left_server, left_share) | Prefix::VerbatimUNC(left_server, left_share),
            Prefix::UNC(right_server, right_share) | Prefix::VerbatimUNC(right_server, right_share),
        ) => {
            windows_os_str_eq_ignore_case(left_server, right_server)
                && windows_os_str_eq_ignore_case(left_share, right_share)
        }
        (Prefix::DeviceNS(left), Prefix::DeviceNS(right))
        | (Prefix::Verbatim(left), Prefix::Verbatim(right)) => {
            windows_os_str_eq_ignore_case(left, right)
        }
        _ => false,
    }
}

#[cfg(windows)]
fn windows_os_str_eq_ignore_case(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Globalization::{CompareStringOrdinal, CSTR_EQUAL};

    let left = left.encode_wide().collect::<Vec<_>>();
    let right = right.encode_wide().collect::<Vec<_>>();
    let (Ok(left_len), Ok(right_len)) = (i32::try_from(left.len()), i32::try_from(right.len()))
    else {
        return false;
    };

    // Counted UTF-16 comparison preserves unpaired surrogates in Windows paths.
    unsafe {
        CompareStringOrdinal(left.as_ptr(), left_len, right.as_ptr(), right_len, 1) == CSTR_EQUAL
    }
}
