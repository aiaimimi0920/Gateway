use std::env;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

mod paths;
pub use paths::validate_state_directory;

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

        let state_paths = paths::resolve_path_views(&state_dir)?;
        let routes_paths = paths::resolve_path_views(&routes_file)?;
        let release_paths = release_payload_root
            .as_deref()
            .map(paths::resolve_path_views)
            .transpose()?;
        if let (Some(release_paths), Some(release_payload_root)) =
            (release_paths.as_ref(), release_payload_root.as_deref())
        {
            paths::validate_state_path_views(
                &state_paths,
                release_paths,
                &state_dir,
                release_payload_root,
            )?;
            paths::validate_routes_path_views(
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
