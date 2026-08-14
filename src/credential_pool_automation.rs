use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use rquest::Method;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};
use url::{Host, Url};
use uuid::Uuid;

use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml};
use crate::state::AppState;

const MAX_DRIVER_OUTPUT_BYTES: usize = 2 * 1024 * 1024;
const DEFAULT_TARGET_SIZE: usize = 1;
const MAX_TARGET_SIZE: usize = 10_000;

#[derive(Clone, Debug)]
pub struct CredentialPoolAutomationConfig {
    pub enabled: bool,
    pub interval_secs: u64,
    pub driver_config_path: Option<PathBuf>,
    pub script_root: Option<PathBuf>,
    pub default_timeout_secs: u64,
    pub refill_queue_enabled: bool,
    pub refill_notification_interval_secs: u64,
    pub refill_task_ttl_secs: u64,
    pub refill_default_lease_secs: u64,
    pub refill_max_lease_secs: u64,
    pub refill_stream_max_len: usize,
}

impl Default for CredentialPoolAutomationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_secs: 60,
            driver_config_path: None,
            script_root: None,
            default_timeout_secs: 60,
            refill_queue_enabled: false,
            refill_notification_interval_secs: 30,
            refill_task_ttl_secs: 7 * 24 * 60 * 60,
            refill_default_lease_secs: 300,
            refill_max_lease_secs: 3_600,
            refill_stream_max_len: 10_000,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DriverRegistryDocument {
    #[serde(default)]
    drivers: Vec<CredentialAutomationDriver>,
}

#[derive(Clone, Debug, Deserialize)]
struct CredentialAutomationDriver {
    id: String,
    #[serde(default)]
    provider_ids: Vec<String>,
    #[serde(default)]
    timeout_secs: Option<u64>,
    #[serde(flatten)]
    transport: CredentialAutomationDriverTransport,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum CredentialAutomationDriverTransport {
    Script {
        script: String,
    },
    Http {
        endpoint: String,
        #[serde(default)]
        secret_env: Option<String>,
    },
}

impl CredentialAutomationDriver {
    fn mode(&self) -> &'static str {
        match self.transport {
            CredentialAutomationDriverTransport::Script { .. } => "script",
            CredentialAutomationDriverTransport::Http { .. } => "http",
        }
    }

    fn supports_provider(&self, provider_id: &str) -> bool {
        self.provider_ids.iter().any(|entry| entry == provider_id)
    }
}

#[derive(Clone, Debug, Default)]
struct DriverRegistry {
    drivers: BTreeMap<String, CredentialAutomationDriver>,
}

impl DriverRegistry {
    fn load(config: &CredentialPoolAutomationConfig) -> anyhow::Result<Self> {
        let Some(path) = config.driver_config_path.as_deref() else {
            return Ok(Self::default());
        };
        let bytes = std::fs::read(path).map_err(|error| {
            anyhow::anyhow!(
                "failed to read credential automation driver registry '{}': {error}",
                path.display()
            )
        })?;
        let document: DriverRegistryDocument = serde_json::from_slice(&bytes).map_err(|error| {
            anyhow::anyhow!(
                "failed to parse credential automation driver registry '{}': {error}",
                path.display()
            )
        })?;
        let mut drivers = BTreeMap::new();
        for mut driver in document.drivers {
            driver.id = normalize_required_id(&driver.id, "driver id")?;
            driver.provider_ids = driver
                .provider_ids
                .into_iter()
                .map(|provider_id| normalize_required_id(&provider_id, "provider id"))
                .collect::<anyhow::Result<Vec<_>>>()?;
            if driver.provider_ids.is_empty() {
                anyhow::bail!(
                    "credential automation driver '{}' has no provider_ids",
                    driver.id
                );
            }
            validate_driver(config, &driver)?;
            if drivers.insert(driver.id.clone(), driver).is_some() {
                anyhow::bail!("duplicate credential automation driver id");
            }
        }
        Ok(Self { drivers })
    }

    fn resolve<'a>(
        &'a self,
        provider: &ProviderConfigYaml,
    ) -> Result<Option<&'a CredentialAutomationDriver>, String> {
        if let Some(explicit_id) = provider
            .credential_automation_driver_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let Some(driver) = self.drivers.get(explicit_id) else {
                return Err(format!(
                    "configured driver '{explicit_id}' is not registered"
                ));
            };
            if !driver.supports_provider(&provider.id) {
                return Err(format!(
                    "driver '{}' is not allowlisted for provider '{}'",
                    driver.id, provider.id
                ));
            }
            return Ok(Some(driver));
        }

        let mut matches = self
            .drivers
            .values()
            .filter(|driver| driver.supports_provider(&provider.id));
        let first = matches.next();
        if matches.next().is_some() {
            return Err(format!(
                "provider '{}' matches multiple drivers; set credential_automation_driver_id",
                provider.id
            ));
        }
        Ok(first)
    }
}

fn normalize_required_id(value: &str, label: &str) -> anyhow::Result<String> {
    let normalized = value.trim();
    if normalized.is_empty()
        || !normalized
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        anyhow::bail!("{label} must contain only ASCII letters, digits, '-' or '_'");
    }
    Ok(normalized.to_string())
}

fn validate_driver(
    config: &CredentialPoolAutomationConfig,
    driver: &CredentialAutomationDriver,
) -> anyhow::Result<()> {
    match &driver.transport {
        CredentialAutomationDriverTransport::Script { script } => {
            resolve_allowlisted_script_path(config, script)?;
        }
        CredentialAutomationDriverTransport::Http {
            endpoint,
            secret_env,
        } => {
            let url = Url::parse(endpoint).map_err(|error| {
                anyhow::anyhow!("driver '{}' endpoint is invalid: {error}", driver.id)
            })?;
            let is_loopback_http = url.scheme() == "http"
                && match url.host() {
                    Some(Host::Ipv4(address)) => address.is_loopback(),
                    Some(Host::Ipv6(address)) => address.is_loopback(),
                    Some(Host::Domain(_)) | None => false,
                };
            if url.scheme() != "https" && !is_loopback_http {
                anyhow::bail!(
                    "driver '{}' endpoint must use HTTPS (HTTP is allowed only for loopback)",
                    driver.id
                );
            }
            if let Some(secret_env) = secret_env {
                normalize_required_id(secret_env, "secret_env")?;
            }
        }
    }
    Ok(())
}

fn resolve_allowlisted_script_path(
    config: &CredentialPoolAutomationConfig,
    script: &str,
) -> anyhow::Result<PathBuf> {
    let root = config.script_root.as_deref().ok_or_else(|| {
        anyhow::anyhow!("script driver requires GATEWAY_CREDENTIAL_POOL_AUTOMATION_SCRIPT_ROOT")
    })?;
    let relative = Path::new(script);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        anyhow::bail!("script driver path must be a relative path without '..'");
    }
    let canonical_root = root.canonicalize().map_err(|error| {
        anyhow::anyhow!(
            "failed to canonicalize script root '{}': {error}",
            root.display()
        )
    })?;
    let canonical_script = canonical_root
        .join(relative)
        .canonicalize()
        .map_err(|error| {
            anyhow::anyhow!(
                "failed to canonicalize automation script '{}': {error}",
                relative.display()
            )
        })?;
    if !canonical_script.starts_with(&canonical_root) || !canonical_script.is_file() {
        anyhow::bail!("automation script is outside the allowlisted root or is not a file");
    }
    Ok(canonical_script)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialPoolDriverView {
    pub id: String,
    pub mode: String,
    pub provider_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialPoolAutomationView {
    pub provider_id: String,
    pub provider_label: String,
    pub target_size: usize,
    pub credential_count: usize,
    pub active_credential_count: usize,
    pub auto_refill_enabled: bool,
    pub auto_prune_enabled: bool,
    pub driver_id: Option<String>,
    pub driver_mode: Option<String>,
    pub driver_configured: bool,
    pub state: String,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub last_action: Option<String>,
    pub created_count: usize,
    pub pruned_count: usize,
    pub message: Option<String>,
    pub revision_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderRunState {
    state: String,
    last_run_at: Option<String>,
    next_run_at: Option<String>,
    last_action: Option<String>,
    created_count: usize,
    pruned_count: usize,
    message: Option<String>,
    revision_id: Option<String>,
}

pub struct CredentialPoolAutomationRuntime {
    config: CredentialPoolAutomationConfig,
    registry: DriverRegistry,
    states: RwLock<BTreeMap<String, ProviderRunState>>,
    provider_locks: DashMap<String, Arc<Mutex<()>>>,
}

impl CredentialPoolAutomationRuntime {
    pub fn load(config: &CredentialPoolAutomationConfig) -> anyhow::Result<Self> {
        let registry = DriverRegistry::load(config)?;
        Ok(Self {
            config: config.clone(),
            registry,
            states: RwLock::new(BTreeMap::new()),
            provider_locks: DashMap::new(),
        })
    }

    pub fn disabled() -> Self {
        Self {
            config: CredentialPoolAutomationConfig::default(),
            registry: DriverRegistry::default(),
            states: RwLock::new(BTreeMap::new()),
            provider_locks: DashMap::new(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn interval(&self) -> Duration {
        Duration::from_secs(self.config.interval_secs.max(5))
    }

    pub fn refill_queue_enabled(&self) -> bool {
        self.config.refill_queue_enabled
    }

    pub fn refill_notification_interval(&self) -> Duration {
        Duration::from_secs(self.config.refill_notification_interval_secs.max(5))
    }

    pub fn refill_task_ttl_seconds(&self) -> u64 {
        self.config.refill_task_ttl_secs.max(300)
    }

    pub fn refill_default_lease_seconds(&self) -> u64 {
        self.config.refill_default_lease_secs.clamp(30, 3_600)
    }

    pub fn refill_max_lease_seconds(&self) -> u64 {
        self.config
            .refill_max_lease_secs
            .clamp(self.refill_default_lease_seconds(), 86_400)
    }

    pub fn refill_stream_max_len(&self) -> usize {
        self.config.refill_stream_max_len.clamp(100, 1_000_000)
    }

    pub fn has_driver_for_provider(&self, provider: &ProviderConfigYaml) -> bool {
        self.registry.resolve(provider).ok().flatten().is_some()
    }

    pub fn drivers(&self) -> Vec<CredentialPoolDriverView> {
        self.registry
            .drivers
            .values()
            .map(|driver| CredentialPoolDriverView {
                id: driver.id.clone(),
                mode: driver.mode().to_string(),
                provider_ids: driver.provider_ids.clone(),
            })
            .collect()
    }

    pub async fn provider_views(
        &self,
        providers: &[ProviderConfigYaml],
    ) -> Vec<ProviderCredentialPoolAutomationView> {
        let states = self.states.read().await;
        providers
            .iter()
            .map(|provider| {
                let resolved_driver = self.registry.resolve(provider);
                let driver = resolved_driver.as_ref().ok().and_then(|value| *value);
                let run_state = states.get(&provider.id);
                let resolution_error = resolved_driver.err();
                let default_state = if !self.config.enabled {
                    "disabled"
                } else if driver.is_none() {
                    "not_configured"
                } else {
                    "idle"
                };
                let default_message = resolution_error.or_else(|| {
                    if self.config.enabled && driver.is_none() {
                        Some("该渠道尚未配置自动补号驱动器".to_string())
                    } else {
                        None
                    }
                });
                ProviderCredentialPoolAutomationView {
                    provider_id: provider.id.clone(),
                    provider_label: provider
                        .label
                        .clone()
                        .unwrap_or_else(|| provider.id.clone()),
                    target_size: normalized_target_size(provider.pool_target_size),
                    credential_count: provider.credentials.len(),
                    active_credential_count: active_credential_count(provider),
                    auto_refill_enabled: provider.auto_refill_enabled,
                    auto_prune_enabled: provider.auto_prune_enabled,
                    driver_id: driver
                        .map(|entry| entry.id.clone())
                        .or_else(|| provider.credential_automation_driver_id.clone()),
                    driver_mode: driver.map(|entry| entry.mode().to_string()),
                    driver_configured: driver.is_some(),
                    state: run_state
                        .map(|state| state.state.clone())
                        .unwrap_or_else(|| default_state.to_string()),
                    last_run_at: run_state.and_then(|state| state.last_run_at.clone()),
                    next_run_at: run_state.and_then(|state| state.next_run_at.clone()),
                    last_action: run_state.and_then(|state| state.last_action.clone()),
                    created_count: run_state.map(|state| state.created_count).unwrap_or(0),
                    pruned_count: run_state.map(|state| state.pruned_count).unwrap_or(0),
                    message: run_state
                        .and_then(|state| state.message.clone())
                        .or(default_message),
                    revision_id: run_state.and_then(|state| state.revision_id.clone()),
                }
            })
            .collect()
    }

    fn lock_for_provider(&self, provider_id: &str) -> Arc<Mutex<()>> {
        self.provider_locks
            .entry(provider_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    async fn set_state(&self, provider_id: &str, state: ProviderRunState) {
        self.states
            .write()
            .await
            .insert(provider_id.to_string(), state);
    }
}

fn normalized_target_size(value: Option<usize>) -> usize {
    value
        .unwrap_or(DEFAULT_TARGET_SIZE)
        .clamp(1, MAX_TARGET_SIZE)
}

fn active_credential_count(provider: &ProviderConfigYaml) -> usize {
    if provider.credentials.is_empty() {
        usize::from(!provider.api_key.trim().is_empty() || provider.auth_token.is_some())
    } else {
        provider
            .credentials
            .iter()
            .filter(|credential| credential.enabled.unwrap_or(true))
            .count()
    }
}

fn identity_category_requested_count(provider: &ProviderConfigYaml) -> usize {
    provider
        .credential_identity_categories
        .iter()
        .filter_map(serde_json::Value::as_object)
        .filter(|category| {
            category
                .get("auto_refill_enabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|category| {
            let category_id = category
                .get("id")
                .and_then(serde_json::Value::as_str)?
                .trim();
            if category_id.is_empty() {
                return None;
            }
            let target_size = category
                .get("pool_target_size")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .map(|value| value.clamp(1, MAX_TARGET_SIZE))
                .unwrap_or(DEFAULT_TARGET_SIZE);
            let active_count = provider
                .credentials
                .iter()
                .filter(|credential| credential.enabled.unwrap_or(true))
                .filter(|credential| {
                    credential.credential_identity_category_id.as_deref() == Some(category_id)
                })
                .count();
            Some(target_size.saturating_sub(active_count))
        })
        .fold(0usize, usize::saturating_add)
        .min(MAX_TARGET_SIZE)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DriverRequest {
    run_id: String,
    action: &'static str,
    provider: DriverProviderContext,
    #[serde(skip_serializing_if = "Option::is_none")]
    refill: Option<DriverRefillContext>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DriverRefillContext {
    task_id: String,
    requested_count: usize,
    artifact_reference: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DriverProviderContext {
    id: String,
    label: String,
    target_size: usize,
    credential_count: usize,
    active_credential_count: usize,
    requested_count: usize,
    auto_refill_enabled: bool,
    auto_prune_enabled: bool,
    identity_categories: Vec<serde_json::Value>,
    credentials: Vec<DriverCredentialContext>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DriverCredentialContext {
    id: String,
    account_name: Option<String>,
    enabled: bool,
    identity_category_id: Option<String>,
}

fn effective_credential_id(
    provider_id: &str,
    index: usize,
    credential: &ProviderCredentialYaml,
) -> String {
    credential
        .id
        .clone()
        .unwrap_or_else(|| format!("{provider_id}-cred-{index}"))
}

fn prune_credentials(provider: &mut ProviderConfigYaml, prune_ids: &HashSet<String>) -> usize {
    let before_prune = provider.credentials.len();
    let provider_id = provider.id.clone();
    provider.credentials = std::mem::take(&mut provider.credentials)
        .into_iter()
        .enumerate()
        .filter_map(|(index, credential)| {
            let credential_id = effective_credential_id(&provider_id, index, &credential);
            (!prune_ids.contains(&credential_id)).then_some(credential)
        })
        .collect();
    before_prune.saturating_sub(provider.credentials.len())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DriverResponse {
    #[serde(default)]
    credentials: Vec<ProviderCredentialYaml>,
    #[serde(default)]
    prune: Vec<DriverPruneDecision>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DriverPruneDecision {
    credential_id: String,
    classification: PermanentPruneClassification,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PermanentPruneClassification {
    PermanentAuthFailure,
    AccountDeleted,
    PermanentUpstreamRejection,
}

pub async fn start_credential_pool_automation_task(state: Arc<AppState>) {
    if !state.credential_pool_automation.enabled() {
        tracing::info!("credential pool automation worker is disabled");
        return;
    }
    loop {
        sweep_credential_pool_automation_once(&state).await;
        tokio::time::sleep(state.credential_pool_automation.interval()).await;
    }
}

pub async fn sweep_credential_pool_automation_once(state: &Arc<AppState>) {
    let provider_ids = state
        .route_config
        .snapshot()
        .document()
        .providers
        .iter()
        .filter(|provider| {
            (provider.auto_refill_enabled || provider.auto_prune_enabled)
                && state
                    .credential_pool_automation
                    .has_driver_for_provider(provider)
        })
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();
    for provider_id in provider_ids {
        if let Err(error) = run_provider_credential_pool_automation(state, &provider_id).await {
            tracing::warn!(provider_id, error = %error, "credential pool automation sweep failed");
        }
    }
}

pub async fn collect_refill_credentials_from_driver(
    state: &Arc<AppState>,
    provider_id: &str,
    task_id: &str,
    requested_count: usize,
    artifact_reference: &str,
) -> anyhow::Result<Vec<ProviderCredentialYaml>> {
    if !state.credential_pool_automation.enabled() {
        anyhow::bail!("credential pool automation is disabled");
    }
    let snapshot = state.route_config.snapshot();
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("provider '{provider_id}' was not found"))?;
    let driver = state
        .credential_pool_automation
        .registry
        .resolve(&provider)
        .map_err(anyhow::Error::msg)?
        .ok_or_else(|| anyhow::anyhow!("该渠道尚未配置自动补号驱动器"))?
        .clone();
    let target_size = normalized_target_size(provider.pool_target_size);
    let active_count = active_credential_count(&provider);
    let request = DriverRequest {
        run_id: Uuid::new_v4().to_string(),
        action: "collect_refill",
        provider: DriverProviderContext {
            id: provider.id.clone(),
            label: provider
                .label
                .clone()
                .unwrap_or_else(|| provider.id.clone()),
            target_size,
            credential_count: provider.credentials.len(),
            active_credential_count: active_count,
            requested_count,
            auto_refill_enabled: provider.auto_refill_enabled,
            auto_prune_enabled: false,
            identity_categories: provider.credential_identity_categories.clone(),
            credentials: provider
                .credentials
                .iter()
                .enumerate()
                .map(|(index, credential)| DriverCredentialContext {
                    id: effective_credential_id(&provider.id, index, credential),
                    account_name: credential.account_name.clone(),
                    enabled: credential.enabled.unwrap_or(true),
                    identity_category_id: credential.credential_identity_category_id.clone(),
                })
                .collect(),
        },
        refill: Some(DriverRefillContext {
            task_id: task_id.to_string(),
            requested_count,
            artifact_reference: artifact_reference.to_string(),
        }),
    };
    let response =
        execute_driver(&state.credential_pool_automation.config, &driver, &request).await?;
    if !response.prune.is_empty() {
        anyhow::bail!("refill collection driver must not return prune decisions");
    }
    Ok(response.credentials)
}

pub async fn run_provider_credential_pool_automation(
    state: &Arc<AppState>,
    provider_id: &str,
) -> anyhow::Result<ProviderCredentialPoolAutomationView> {
    if !state.credential_pool_automation.enabled() {
        anyhow::bail!("credential pool automation is disabled");
    }
    let provider_lock = state
        .credential_pool_automation
        .lock_for_provider(provider_id);
    let _guard = provider_lock.lock().await;
    let snapshot = state.route_config.snapshot();
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("provider '{provider_id}' was not found"))?;
    if !provider.auto_refill_enabled && !provider.auto_prune_enabled {
        anyhow::bail!("provider '{provider_id}' has not enabled automatic refill or prune");
    }
    let driver = state
        .credential_pool_automation
        .registry
        .resolve(&provider)
        .map_err(anyhow::Error::msg)?
        .ok_or_else(|| anyhow::anyhow!("该渠道尚未配置自动补号驱动器"))?
        .clone();
    let started_at = now_rfc3339();
    state
        .credential_pool_automation
        .set_state(
            provider_id,
            ProviderRunState {
                state: "running".to_string(),
                last_run_at: Some(started_at.clone()),
                last_action: Some("reconcile".to_string()),
                ..ProviderRunState::default()
            },
        )
        .await;

    let result = reconcile_provider_with_driver(state, &snapshot, provider, &driver).await;
    let final_state = match &result {
        Ok(outcome) => ProviderRunState {
            state: "succeeded".to_string(),
            last_run_at: Some(started_at),
            next_run_at: Some(next_run_at(state.credential_pool_automation.interval())),
            last_action: Some("reconcile".to_string()),
            created_count: outcome.created_count,
            pruned_count: outcome.pruned_count,
            message: outcome.message.clone(),
            revision_id: outcome.revision_id.clone(),
        },
        Err(error) => ProviderRunState {
            state: "failed".to_string(),
            last_run_at: Some(started_at),
            next_run_at: Some(next_run_at(state.credential_pool_automation.interval())),
            last_action: Some("reconcile".to_string()),
            message: Some(sanitize_driver_error(error)),
            ..ProviderRunState::default()
        },
    };
    state
        .credential_pool_automation
        .set_state(provider_id, final_state)
        .await;
    result?;

    state
        .credential_pool_automation
        .provider_views(&state.route_config.snapshot().document().providers)
        .await
        .into_iter()
        .find(|view| view.provider_id == provider_id)
        .ok_or_else(|| anyhow::anyhow!("provider automation status disappeared"))
}

#[derive(Debug)]
struct ReconcileOutcome {
    created_count: usize,
    pruned_count: usize,
    message: Option<String>,
    revision_id: Option<String>,
}

async fn reconcile_provider_with_driver(
    state: &Arc<AppState>,
    snapshot: &crate::routing::config::RouteConfigSnapshot,
    provider: ProviderConfigYaml,
    driver: &CredentialAutomationDriver,
) -> anyhow::Result<ReconcileOutcome> {
    let target_size = normalized_target_size(provider.pool_target_size);
    let active_count = active_credential_count(&provider);
    let requested_count = if provider.auto_refill_enabled {
        target_size
            .saturating_sub(active_count)
            .max(identity_category_requested_count(&provider))
    } else {
        0
    };
    let request = DriverRequest {
        run_id: Uuid::new_v4().to_string(),
        action: "reconcile",
        provider: DriverProviderContext {
            id: provider.id.clone(),
            label: provider
                .label
                .clone()
                .unwrap_or_else(|| provider.id.clone()),
            target_size,
            credential_count: provider.credentials.len(),
            active_credential_count: active_count,
            requested_count,
            auto_refill_enabled: provider.auto_refill_enabled,
            auto_prune_enabled: provider.auto_prune_enabled,
            identity_categories: provider.credential_identity_categories.clone(),
            credentials: provider
                .credentials
                .iter()
                .enumerate()
                .map(|(index, credential)| DriverCredentialContext {
                    id: effective_credential_id(&provider.id, index, credential),
                    account_name: credential.account_name.clone(),
                    enabled: credential.enabled.unwrap_or(true),
                    identity_category_id: credential.credential_identity_category_id.clone(),
                })
                .collect(),
        },
        refill: None,
    };
    let response =
        execute_driver(&state.credential_pool_automation.config, driver, &request).await?;
    let mut document = snapshot.document().clone();
    let provider_index = document
        .providers
        .iter()
        .position(|candidate| candidate.id == provider.id)
        .ok_or_else(|| anyhow::anyhow!("provider disappeared from route document"))?;
    let target_provider = &mut document.providers[provider_index];
    let mut existing_ids = target_provider
        .credentials
        .iter()
        .enumerate()
        .map(|(index, credential)| effective_credential_id(&target_provider.id, index, credential))
        .collect::<HashSet<_>>();

    let mut created_count = 0usize;
    if provider.auto_refill_enabled {
        for credential in response.credentials.into_iter().take(requested_count) {
            let credential_id = credential
                .id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("driver returned a credential without an id"))?
                .to_string();
            normalize_required_id(&credential_id, "credential id")?;
            if !existing_ids.insert(credential_id) {
                anyhow::bail!("driver returned a duplicate credential id");
            }
            target_provider.credentials.push(credential);
            created_count += 1;
        }
    }

    let mut prune_ids = HashSet::new();
    if provider.auto_prune_enabled {
        for decision in response.prune {
            let _classification = decision.classification;
            if existing_ids.contains(&decision.credential_id) {
                prune_ids.insert(decision.credential_id);
            }
        }
    }
    let pruned_count = prune_credentials(target_provider, &prune_ids);

    if created_count == 0 && pruned_count == 0 {
        return Ok(ReconcileOutcome {
            created_count,
            pruned_count,
            message: response
                .message
                .or_else(|| Some("凭证池已检查，无需修改。".to_string())),
            revision_id: Some(snapshot.revision().id().to_string()),
        });
    }
    let runtime = state
        .route_config_runtime
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("route configuration runtime is unavailable"))?;
    let committed = runtime
        .commit_automation_document(
            snapshot.revision().id(),
            document,
            Some(format!(
                "credential pool automation for {}: +{}, -{}",
                provider.id, created_count, pruned_count
            )),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(ReconcileOutcome {
        created_count,
        pruned_count,
        message: response.message.or_else(|| {
            Some(format!(
                "自动补号 {created_count} 个，自动剔号 {pruned_count} 个。"
            ))
        }),
        revision_id: Some(committed.revision().id().to_string()),
    })
}

async fn execute_driver(
    config: &CredentialPoolAutomationConfig,
    driver: &CredentialAutomationDriver,
    request: &DriverRequest,
) -> anyhow::Result<DriverResponse> {
    let timeout = Duration::from_secs(
        driver
            .timeout_secs
            .unwrap_or(config.default_timeout_secs)
            .clamp(1, 900),
    );
    let bytes = match &driver.transport {
        CredentialAutomationDriverTransport::Script { script } => {
            execute_script_driver(config, script, request, timeout).await?
        }
        CredentialAutomationDriverTransport::Http {
            endpoint,
            secret_env,
        } => execute_http_driver(endpoint, secret_env.as_deref(), request, timeout).await?,
    };
    if bytes.len() > MAX_DRIVER_OUTPUT_BYTES {
        anyhow::bail!("credential automation driver response exceeded the size limit");
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("credential automation driver returned invalid JSON"))
}

async fn execute_script_driver(
    config: &CredentialPoolAutomationConfig,
    script: &str,
    request: &DriverRequest,
    timeout: Duration,
) -> anyhow::Result<Vec<u8>> {
    let script_path = resolve_allowlisted_script_path(config, script)?;
    let extension = script_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut command = match extension.as_str() {
        "ps1" => {
            #[cfg(target_os = "windows")]
            let mut command = Command::new("powershell");
            #[cfg(not(target_os = "windows"))]
            let mut command = Command::new("pwsh");
            command.args(["-NoProfile", "-NonInteractive"]);
            #[cfg(target_os = "windows")]
            command.args(["-ExecutionPolicy", "Bypass"]);
            command.arg("-File");
            command.arg(&script_path);
            command
        }
        "js" | "mjs" | "cjs" => {
            let mut command = Command::new("node");
            command.arg(&script_path);
            command
        }
        "py" => {
            #[cfg(target_os = "windows")]
            let mut command = Command::new("python");
            #[cfg(not(target_os = "windows"))]
            let mut command = Command::new("python3");
            command.arg(&script_path);
            command
        }
        "exe" => Command::new(&script_path),
        _ => anyhow::bail!("unsupported automation script extension"),
    };
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|_| anyhow::anyhow!("failed to start credential automation script"))?;
    let request_bytes = serde_json::to_vec(request)?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&request_bytes)
            .await
            .map_err(|_| anyhow::anyhow!("failed to write credential automation script input"))?;
    }
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| anyhow::anyhow!("credential automation script timed out"))?
        .map_err(|_| anyhow::anyhow!("credential automation script failed to exit cleanly"))?;
    if !output.status.success() {
        anyhow::bail!("credential automation script returned a non-zero exit status");
    }
    Ok(output.stdout)
}

async fn execute_http_driver(
    endpoint: &str,
    secret_env: Option<&str>,
    request: &DriverRequest,
    timeout: Duration,
) -> anyhow::Result<Vec<u8>> {
    let client = rquest::Client::builder().timeout(timeout).build()?;
    let mut builder = client.request(Method::POST, endpoint).json(request);
    if let Some(secret_env) = secret_env {
        let token = std::env::var(secret_env)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("credential automation HTTP secret is unavailable"))?;
        builder = builder.bearer_auth(token);
    }
    let response = builder
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("credential automation HTTP request failed"))?;
    if !response.status().is_success() {
        anyhow::bail!("credential automation HTTP endpoint returned a non-success status");
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| anyhow::anyhow!("failed to read credential automation HTTP response"))?;
    Ok(bytes.to_vec())
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}

fn next_run_at(interval: Duration) -> String {
    let seconds = i64::try_from(interval.as_secs()).unwrap_or(i64::MAX);
    OffsetDateTime::now_utc()
        .saturating_add(time::Duration::seconds(seconds))
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}

fn sanitize_driver_error(error: &anyhow::Error) -> String {
    let message = error.to_string();
    if message.chars().count() <= 320 {
        message
    } else {
        format!("{}...", message.chars().take(320).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::post;
    use axum::{Json, Router};

    #[test]
    fn registry_document_accepts_the_documented_tagged_transport_and_rejects_unknown_fields() {
        let documented: DriverRegistryDocument = serde_json::from_value(serde_json::json!({
            "drivers": [
                {
                    "id": "provider-a-driver",
                    "provider_ids": ["provider-a"],
                    "timeout_secs": 30,
                    "mode": "script",
                    "script": "provider-a.mjs"
                }
            ]
        }))
        .unwrap();
        assert!(matches!(
            documented.drivers[0].transport,
            CredentialAutomationDriverTransport::Script { .. }
        ));

        let unknown = serde_json::from_value::<DriverRegistryDocument>(serde_json::json!({
            "drivers": [
                {
                    "id": "provider-a-driver",
                    "provider_ids": ["provider-a"],
                    "mode": "script",
                    "script": "provider-a.mjs",
                    "command": "not-allowlisted"
                }
            ]
        }));
        assert!(unknown.is_err());
    }

    #[test]
    fn registry_rejects_non_https_non_loopback_http() {
        let config = CredentialPoolAutomationConfig::default();
        let driver = CredentialAutomationDriver {
            id: "unsafe-http".to_string(),
            provider_ids: vec!["provider-a".to_string()],
            timeout_secs: None,
            transport: CredentialAutomationDriverTransport::Http {
                endpoint: "http://example.com/refill".to_string(),
                secret_env: None,
            },
        };

        assert!(validate_driver(&config, &driver).is_err());
    }

    #[test]
    fn registry_rejects_plaintext_localhost_domains() {
        let config = CredentialPoolAutomationConfig::default();
        let driver = CredentialAutomationDriver {
            id: "unsafe-localhost-domain".to_string(),
            provider_ids: vec!["provider-a".to_string()],
            timeout_secs: None,
            transport: CredentialAutomationDriverTransport::Http {
                endpoint: "http://localhost:9911/refill".to_string(),
                secret_env: None,
            },
        };

        assert!(validate_driver(&config, &driver).is_err());
    }

    #[test]
    fn registry_resolves_only_allowlisted_provider() {
        let driver = CredentialAutomationDriver {
            id: "provider-a-driver".to_string(),
            provider_ids: vec!["provider-a".to_string()],
            timeout_secs: None,
            transport: CredentialAutomationDriverTransport::Http {
                endpoint: "http://127.0.0.1:9911/refill".to_string(),
                secret_env: None,
            },
        };
        let registry = DriverRegistry {
            drivers: BTreeMap::from([(driver.id.clone(), driver)]),
        };
        let mut provider: ProviderConfigYaml = serde_json::from_value(serde_json::json!({
            "id": "provider-a",
            "label": "Provider A",
            "base_url": "https://api.example.com",
            "credentials": []
        }))
        .unwrap();

        assert_eq!(
            registry.resolve(&provider).unwrap().unwrap().id,
            "provider-a-driver"
        );
        provider.id = "provider-b".to_string();
        assert!(registry.resolve(&provider).unwrap().is_none());
    }

    #[test]
    fn driver_response_accepts_only_permanent_prune_classifications() {
        let quota_response = br#"{
            "credentials": [],
            "prune": [{"credential_id":"account-a","classification":"quota_exhausted"}]
        }"#;
        let permanent_response = br#"{
            "credentials": [],
            "prune": [{"credential_id":"account-a","classification":"permanent_auth_failure"}]
        }"#;

        assert!(serde_json::from_slice::<DriverResponse>(quota_response).is_err());
        assert!(serde_json::from_slice::<DriverResponse>(permanent_response).is_ok());
    }

    #[test]
    fn prune_removes_legacy_credential_by_its_synthetic_id() {
        let mut provider: ProviderConfigYaml = serde_json::from_value(serde_json::json!({
            "id": "provider-a",
            "label": "Provider A",
            "base_url": "https://api.example.com",
            "credentials": [{"account_name": "Legacy account"}]
        }))
        .unwrap();
        let prune_ids = HashSet::from(["provider-a-cred-0".to_string()]);

        assert_eq!(prune_credentials(&mut provider, &prune_ids), 1);
        assert!(provider.credentials.is_empty());
    }

    #[test]
    fn identity_category_refill_contributes_to_the_driver_request_count() {
        let provider: ProviderConfigYaml = serde_json::from_value(serde_json::json!({
            "id": "codex",
            "label": "Codex",
            "base_url": "https://chatgpt.com/backend-api",
            "auto_refill_enabled": true,
            "credential_identity_categories": [
                {
                    "id": "plus",
                    "label": "Plus",
                    "pool_target_size": 3,
                    "auto_refill_enabled": true
                },
                {
                    "id": "free",
                    "label": "Free",
                    "pool_target_size": 10,
                    "auto_refill_enabled": false
                }
            ],
            "credentials": [
                {
                    "id": "codex-plus-1",
                    "credential_identity_category_id": "plus"
                },
                {
                    "id": "codex-free-1",
                    "credential_identity_category_id": "free"
                }
            ]
        }))
        .unwrap();

        assert_eq!(identity_category_requested_count(&provider), 2);
    }

    #[tokio::test]
    async fn http_driver_receives_fixed_context_and_returns_credential_drafts() {
        let app = Router::new().route(
            "/reconcile",
            post(|Json(request): Json<serde_json::Value>| async move {
                assert_eq!(request["action"], "reconcile");
                assert_eq!(request["provider"]["id"], "provider-a");
                assert_eq!(request["provider"]["requestedCount"], 1);
                Json(serde_json::json!({
                    "credentials": [{
                        "id": "provider-a-account-2",
                        "account_name": "Account 2",
                        "api_key": "driver-secret"
                    }],
                    "prune": [],
                    "message": "created one account"
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let driver = CredentialAutomationDriver {
            id: "provider-a-driver".to_string(),
            provider_ids: vec!["provider-a".to_string()],
            timeout_secs: Some(5),
            transport: CredentialAutomationDriverTransport::Http {
                endpoint: format!("http://{address}/reconcile"),
                secret_env: None,
            },
        };
        let request = DriverRequest {
            run_id: "test-run".to_string(),
            action: "reconcile",
            provider: DriverProviderContext {
                id: "provider-a".to_string(),
                label: "Provider A".to_string(),
                target_size: 2,
                credential_count: 1,
                active_credential_count: 1,
                requested_count: 1,
                auto_refill_enabled: true,
                auto_prune_enabled: false,
                identity_categories: vec![],
                credentials: vec![DriverCredentialContext {
                    id: "provider-a-account-1".to_string(),
                    account_name: Some("Account 1".to_string()),
                    enabled: true,
                    identity_category_id: None,
                }],
            },
            refill: None,
        };

        let response = execute_driver(
            &CredentialPoolAutomationConfig::default(),
            &driver,
            &request,
        )
        .await
        .unwrap();

        assert_eq!(response.credentials.len(), 1);
        assert_eq!(
            response.credentials[0].id.as_deref(),
            Some("provider-a-account-2")
        );
        server.abort();
    }
}
