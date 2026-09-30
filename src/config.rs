use std::env;
use std::path::PathBuf;

use crate::console::ConsoleConfig;
use crate::credential_pool_automation::CredentialPoolAutomationConfig;
mod management;
mod storage;
pub use storage::GatewayStorageMode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GatewayRuntimeRole {
    Splitter,
    Worker,
    Standalone,
}

impl GatewayRuntimeRole {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "splitter" => Ok(Self::Splitter),
            "worker" => Ok(Self::Worker),
            "standalone" => Ok(Self::Standalone),
            other => Err(format!(
                "Invalid value for GATEWAY_RUNTIME_ROLE: {} (expected splitter, worker, or standalone)",
                other
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub console: ConsoleConfig,
    pub runtime_role: GatewayRuntimeRole,
    pub storage_mode: GatewayStorageMode,
    pub port: u16,
    pub redis_url: String,
    pub database_url: Option<String>,
    pub upstream_timeout_secs: u64,
    pub max_request_body_bytes: usize,
    pub max_body_chat_completions_bytes: usize,
    pub max_body_completions_bytes: usize,
    pub max_body_messages_bytes: usize,
    pub max_body_responses_bytes: usize,
    pub max_body_embeddings_bytes: usize,
    pub max_body_audio_transcriptions_bytes: usize,
    pub max_body_audio_speech_bytes: usize,
    pub max_body_search_bytes: usize,
    pub max_body_fetch_bytes: usize,
    pub max_body_research_bytes: usize,
    pub max_body_images_generations_bytes: usize,
    pub max_body_images_edits_bytes: usize,
    pub max_body_music_bytes: usize,
    pub max_body_videos_bytes: usize,
    pub response_cache_ttl_secs: u64,
    pub response_cache_max_size_bytes: usize,
    pub quota_pre_deduct_estimate_ratio: f64,
    pub usage_report_batch_size: usize,
    pub provider_probe_interval_secs: u64,
    pub log_level: String,
    /// Optional gateway-level API key for simple shared-secret authentication.
    /// Read from `GATEWAY_API_KEY` env var. `None` means dev mode (no auth).
    pub gateway_api_key: Option<String>,
    /// HMAC secret used to mint and verify project-scoped `neuro_*` keys.
    /// Legacy `new_api_*` tokens remain parseable for compatibility.
    pub gateway_api_key_secret: Option<String>,
    /// Optional management token for internal operator-style routes.
    pub gateway_management_token: Option<String>,
    /// Optional bearer token for keepalive steward routes. When unset, the
    /// local keepalive ensure endpoint remains open for cluster-internal use.
    pub gateway_keepalive_bearer_token: Option<String>,
    /// Default project used when issuing user credentials without an explicit
    /// project binding.
    pub default_project_id: String,
    /// Additional inbound API key header aliases accepted by public routes.
    /// Values are normalized to lowercase header names.
    pub gateway_inbound_api_key_header_aliases: Vec<String>,
    /// Optional file-system synchronization mode for provider credentials.
    pub provider_credential_folder_sync_enabled: bool,
    pub provider_credential_folder_sync_root_dir: Option<String>,
    pub provider_credential_folder_sync_interval_secs: u64,
    pub provider_credential_folder_sync_watch_enabled: bool,
    pub provider_credential_folder_sync_watch_debounce_millis: u64,
    pub provider_credential_folder_sync_import_enabled: bool,
    pub provider_credential_folder_sync_export_enabled: bool,
    pub provider_credential_folder_sync_delete_missing: bool,
    pub provider_credential_refresh_enabled: bool,
    pub provider_credential_refresh_interval_secs: u64,
    pub provider_credential_refresh_before_secs: u64,
    pub provider_credential_refresh_batch_limit: i64,
    pub provider_credential_refresh_lock_ttl_secs: u64,
    pub credential_stock_monitor_enabled: bool,
    pub credential_stock_monitor_interval_secs: u64,
    pub credential_pool_automation: CredentialPoolAutomationConfig,
    pub splitter_worker_executable_path: Option<String>,
    pub splitter_initial_worker_port: u16,
    pub splitter_ready_timeout_secs: u64,
    pub splitter_ready_poll_interval_millis: u64,
    pub splitter_reload_shutdown_timeout_secs: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let console = ConsoleConfig::from_env().map_err(|error| error.to_string())?;
        let storage_mode = GatewayStorageMode::from_env()?;
        let runtime_role = env::var("GATEWAY_RUNTIME_ROLE")
            .ok()
            .as_deref()
            .map(GatewayRuntimeRole::parse)
            .transpose()?
            .unwrap_or(if storage_mode == GatewayStorageMode::Local {
                GatewayRuntimeRole::Standalone
            } else {
                GatewayRuntimeRole::Splitter
            });
        if storage_mode == GatewayStorageMode::Local
            && runtime_role != GatewayRuntimeRole::Standalone
        {
            return Err("Local storage requires standalone runtime role".into());
        }
        let port = parse_env_or("PORT", 4200u16)?;
        let redis_url = if storage_mode == GatewayStorageMode::Local {
            String::new()
        } else {
            env::var("GATEWAY_REDIS_URL")
                .map_err(|_| "GATEWAY_REDIS_URL is required but not set".to_string())?
        };
        let database_url = env::var("GATEWAY_DATABASE_URL")
            .ok()
            .or_else(|| env::var("DATABASE_URL").ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let upstream_timeout_secs = parse_env_or("GATEWAY_UPSTREAM_TIMEOUT_SECS", 120u64)?;
        let max_request_body_bytes =
            parse_env_or("GATEWAY_MAX_REQUEST_BODY_BYTES", 50 * 1024 * 1024usize)?;
        let max_body_chat_completions_bytes =
            parse_env_or("GATEWAY_MAX_BODY_CHAT_COMPLETIONS", max_request_body_bytes)?;
        let max_body_completions_bytes = parse_env_or(
            "GATEWAY_MAX_BODY_COMPLETIONS",
            max_body_chat_completions_bytes,
        )?;
        let max_body_messages_bytes =
            parse_env_or("GATEWAY_MAX_BODY_MESSAGES", max_body_chat_completions_bytes)?;
        let max_body_responses_bytes = parse_env_or(
            "GATEWAY_MAX_BODY_RESPONSES",
            max_body_chat_completions_bytes,
        )?;
        let max_body_embeddings_bytes = parse_env_or(
            "GATEWAY_MAX_BODY_EMBEDDINGS",
            max_body_chat_completions_bytes,
        )?;
        let max_body_audio_transcriptions_bytes = parse_env_or(
            "GATEWAY_MAX_BODY_AUDIO_TRANSCRIPTIONS",
            50 * 1024 * 1024usize,
        )?;
        let max_body_audio_speech_bytes = parse_env_or(
            "GATEWAY_MAX_BODY_AUDIO_SPEECH",
            max_body_chat_completions_bytes,
        )?;
        let max_body_search_bytes = parse_env_or("GATEWAY_MAX_BODY_SEARCH", 5 * 1024 * 1024usize)?;
        let max_body_fetch_bytes = parse_env_or("GATEWAY_MAX_BODY_FETCH", max_body_search_bytes)?;
        let max_body_research_bytes =
            parse_env_or("GATEWAY_MAX_BODY_RESEARCH", max_body_search_bytes)?;
        let max_body_images_generations_bytes =
            parse_env_or("GATEWAY_MAX_BODY_IMAGES_GENERATIONS", 10 * 1024 * 1024usize)?;
        let max_body_images_edits_bytes = parse_env_or(
            "GATEWAY_MAX_BODY_IMAGES_EDITS",
            max_body_chat_completions_bytes,
        )?;
        let max_body_music_bytes = parse_env_or("GATEWAY_MAX_BODY_MUSIC", 10 * 1024 * 1024usize)?;
        let max_body_videos_bytes = parse_env_or("GATEWAY_MAX_BODY_VIDEOS", 10 * 1024 * 1024usize)?;
        let response_cache_ttl_secs = parse_env_or("GATEWAY_RESPONSE_CACHE_TTL_SECS", 300u64)?;
        let response_cache_max_size_bytes =
            parse_env_or("GATEWAY_RESPONSE_CACHE_MAX_SIZE_BYTES", 512 * 1024usize)?;
        let quota_pre_deduct_estimate_ratio =
            parse_env_f64_or("GATEWAY_QUOTA_PRE_DEDUCT_ESTIMATE_RATIO", 1.2)?;
        let usage_report_batch_size = parse_env_or("GATEWAY_USAGE_REPORT_BATCH_SIZE", 100usize)?;
        let provider_probe_interval_secs =
            parse_env_or("GATEWAY_PROVIDER_PROBE_INTERVAL_SECS", 30u64)?;
        let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let gateway_api_key = env::var("GATEWAY_API_KEY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let gateway_api_key_secret = env::var("GATEWAY_API_KEY_SECRET")
            .ok()
            .or_else(|| env::var("AI_GATEWAY_API_KEY_SECRET").ok())
            .or_else(|| env::var("API_KEY_SECRET").ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let gateway_management_token = management::resolve_management_token(
            env::var("GATEWAY_MANAGEMENT_TOKEN").ok(),
            console.state_dir.join("console/admin.json").exists(),
        );
        let gateway_keepalive_bearer_token = env::var("GATEWAY_KEEPALIVE_BEARER_TOKEN")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let default_project_id = env::var("GATEWAY_DEFAULT_PROJECT_ID")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "platform-default-project".to_string());
        let gateway_inbound_api_key_header_aliases =
            parse_csv_headers("GATEWAY_INBOUND_API_KEY_HEADER_ALIASES");
        let provider_credential_folder_sync_enabled =
            parse_env_or("GATEWAY_PROVIDER_CREDENTIAL_FOLDER_SYNC_ENABLED", false)?;
        let provider_credential_folder_sync_root_dir =
            env::var("GATEWAY_PROVIDER_CREDENTIAL_FOLDER_SYNC_ROOT_DIR")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .or_else(default_provider_credential_folder_sync_root_dir);
        let provider_credential_folder_sync_interval_secs = parse_env_or(
            "GATEWAY_PROVIDER_CREDENTIAL_FOLDER_SYNC_INTERVAL_SECS",
            30u64,
        )?;
        let provider_credential_folder_sync_watch_enabled = parse_env_or(
            "GATEWAY_PROVIDER_CREDENTIAL_FOLDER_SYNC_WATCH_ENABLED",
            true,
        )?;
        let provider_credential_folder_sync_watch_debounce_millis = parse_env_or(
            "GATEWAY_PROVIDER_CREDENTIAL_FOLDER_SYNC_WATCH_DEBOUNCE_MILLIS",
            1500u64,
        )?;
        let provider_credential_folder_sync_import_enabled = parse_env_or(
            "GATEWAY_PROVIDER_CREDENTIAL_FOLDER_SYNC_IMPORT_ENABLED",
            true,
        )?;
        let provider_credential_folder_sync_export_enabled = parse_env_or(
            "GATEWAY_PROVIDER_CREDENTIAL_FOLDER_SYNC_EXPORT_ENABLED",
            true,
        )?;
        let provider_credential_folder_sync_delete_missing = parse_env_or(
            "GATEWAY_PROVIDER_CREDENTIAL_FOLDER_SYNC_DELETE_MISSING",
            false,
        )?;
        let provider_credential_refresh_enabled =
            parse_env_or("GATEWAY_PROVIDER_CREDENTIAL_REFRESH_ENABLED", true)?;
        let provider_credential_refresh_interval_secs =
            parse_env_or("GATEWAY_PROVIDER_CREDENTIAL_REFRESH_INTERVAL_SECS", 3600u64)?;
        let provider_credential_refresh_before_secs = parse_env_or(
            "GATEWAY_PROVIDER_CREDENTIAL_REFRESH_BEFORE_SECS",
            24 * 60 * 60u64,
        )?;
        let provider_credential_refresh_batch_limit =
            parse_env_or("GATEWAY_PROVIDER_CREDENTIAL_REFRESH_BATCH_LIMIT", 100i64)?;
        let provider_credential_refresh_lock_ttl_secs =
            parse_env_or("GATEWAY_PROVIDER_CREDENTIAL_REFRESH_LOCK_TTL_SECS", 300u64)?;
        let credential_stock_monitor_enabled =
            parse_env_or("GATEWAY_CREDENTIAL_STOCK_MONITOR_ENABLED", true)?;
        let credential_stock_monitor_interval_secs =
            parse_env_or("GATEWAY_CREDENTIAL_STOCK_MONITOR_INTERVAL_SECS", 60u64)?;
        let credential_pool_automation = CredentialPoolAutomationConfig {
            enabled: parse_env_or("GATEWAY_CREDENTIAL_POOL_AUTOMATION_ENABLED", true)?,
            interval_secs: parse_env_or("GATEWAY_CREDENTIAL_POOL_AUTOMATION_INTERVAL_SECS", 60u64)?,
            driver_config_path: parse_optional_path_env(
                "GATEWAY_CREDENTIAL_POOL_AUTOMATION_DRIVER_CONFIG",
            ),
            script_root: parse_optional_path_env("GATEWAY_CREDENTIAL_POOL_AUTOMATION_SCRIPT_ROOT"),
            default_timeout_secs: parse_env_or(
                "GATEWAY_CREDENTIAL_POOL_AUTOMATION_TIMEOUT_SECS",
                60u64,
            )?,
            refill_queue_enabled: parse_env_or("GATEWAY_CREDENTIAL_REFILL_QUEUE_ENABLED", true)?,
            refill_notification_interval_secs: parse_env_or(
                "GATEWAY_CREDENTIAL_REFILL_NOTIFICATION_INTERVAL_SECS",
                30u64,
            )?,
            refill_task_ttl_secs: parse_env_or(
                "GATEWAY_CREDENTIAL_REFILL_TASK_TTL_SECS",
                7 * 24 * 60 * 60u64,
            )?,
            refill_default_lease_secs: parse_env_or(
                "GATEWAY_CREDENTIAL_REFILL_DEFAULT_LEASE_SECS",
                300u64,
            )?,
            refill_max_lease_secs: parse_env_or(
                "GATEWAY_CREDENTIAL_REFILL_MAX_LEASE_SECS",
                3_600u64,
            )?,
            refill_stream_max_len: parse_env_or(
                "GATEWAY_CREDENTIAL_REFILL_STREAM_MAX_LEN",
                10_000usize,
            )?,
        };
        let splitter_worker_executable_path = env::var("GATEWAY_SPLITTER_WORKER_EXECUTABLE_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let splitter_initial_worker_port = parse_env_or(
            "GATEWAY_SPLITTER_INITIAL_WORKER_PORT",
            port.saturating_add(1),
        )?;
        let splitter_ready_timeout_secs =
            parse_env_or("GATEWAY_SPLITTER_READY_TIMEOUT_SECS", 120u64)?;
        let splitter_ready_poll_interval_millis =
            parse_env_or("GATEWAY_SPLITTER_READY_POLL_INTERVAL_MILLIS", 500u64)?;
        let splitter_reload_shutdown_timeout_secs =
            parse_env_or("GATEWAY_SPLITTER_RELOAD_SHUTDOWN_TIMEOUT_SECS", 600u64)?;

        Ok(Self {
            console,
            runtime_role,
            storage_mode,
            port,
            redis_url,
            database_url,
            upstream_timeout_secs,
            max_request_body_bytes,
            max_body_chat_completions_bytes,
            max_body_completions_bytes,
            max_body_messages_bytes,
            max_body_responses_bytes,
            max_body_embeddings_bytes,
            max_body_audio_transcriptions_bytes,
            max_body_audio_speech_bytes,
            max_body_search_bytes,
            max_body_fetch_bytes,
            max_body_research_bytes,
            max_body_images_generations_bytes,
            max_body_images_edits_bytes,
            max_body_music_bytes,
            max_body_videos_bytes,
            response_cache_ttl_secs,
            response_cache_max_size_bytes,
            quota_pre_deduct_estimate_ratio,
            usage_report_batch_size,
            provider_probe_interval_secs,
            log_level,
            gateway_api_key,
            gateway_api_key_secret,
            gateway_management_token,
            gateway_keepalive_bearer_token,
            default_project_id,
            gateway_inbound_api_key_header_aliases,
            provider_credential_folder_sync_enabled,
            provider_credential_folder_sync_root_dir,
            provider_credential_folder_sync_interval_secs,
            provider_credential_folder_sync_watch_enabled,
            provider_credential_folder_sync_watch_debounce_millis,
            provider_credential_folder_sync_import_enabled,
            provider_credential_folder_sync_export_enabled,
            provider_credential_folder_sync_delete_missing,
            provider_credential_refresh_enabled,
            provider_credential_refresh_interval_secs,
            provider_credential_refresh_before_secs,
            provider_credential_refresh_batch_limit,
            provider_credential_refresh_lock_ttl_secs,
            credential_stock_monitor_enabled,
            credential_stock_monitor_interval_secs,
            credential_pool_automation,
            splitter_worker_executable_path,
            splitter_initial_worker_port,
            splitter_ready_timeout_secs,
            splitter_ready_poll_interval_millis,
            splitter_reload_shutdown_timeout_secs,
        })
    }
}

fn parse_env_or<T>(key: &str, default: T) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(val) => val
            .parse::<T>()
            .map_err(|e| format!("Invalid value for {}: {}", key, e)),
        Err(_) => Ok(default),
    }
}

fn parse_env_f64_or(key: &str, default: f64) -> Result<f64, String> {
    match env::var(key) {
        Ok(val) => val
            .parse::<f64>()
            .map_err(|e| format!("Invalid value for {}: {}", key, e)),
        Err(_) => Ok(default),
    }
}

fn parse_csv_headers(key: &str) -> Vec<String> {
    env::var(key)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(|entry| entry.to_ascii_lowercase())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn default_provider_credential_folder_sync_root_dir() -> Option<String> {
    resolve_home_dir().map(|home_dir| home_dir.join(".neuro").to_string_lossy().into_owned())
}

fn parse_optional_path_env(key: &str) -> Option<PathBuf> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn from_env_treats_blank_gateway_api_key_as_none() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var("GATEWAY_REDIS_URL", "redis://127.0.0.1:6379/0");
        std::env::set_var("GATEWAY_API_KEY", "   ");

        let config = Config::from_env().expect("config should load");
        assert_eq!(config.gateway_api_key, None);

        std::env::remove_var("GATEWAY_API_KEY");
        std::env::remove_var("GATEWAY_REDIS_URL");
    }

    #[test]
    fn from_env_trims_gateway_api_key() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var("GATEWAY_REDIS_URL", "redis://127.0.0.1:6379/0");
        std::env::set_var("GATEWAY_API_KEY", "  temp-public-key  ");

        let config = Config::from_env().expect("config should load");
        assert_eq!(config.gateway_api_key.as_deref(), Some("temp-public-key"));

        std::env::remove_var("GATEWAY_API_KEY");
        std::env::remove_var("GATEWAY_REDIS_URL");
    }
}

fn resolve_home_dir() -> Option<PathBuf> {
    env::var("USERPROFILE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var("HOME")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .or_else(|| {
            let drive = env::var("HOMEDRIVE")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let path = env::var("HOMEPATH")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            match (drive, path) {
                (Some(drive), Some(path)) => Some(PathBuf::from(format!("{drive}{path}"))),
                _ => None,
            }
        })
}
