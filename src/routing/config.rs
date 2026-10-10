// ---------------------------------------------------------------------------
// routing/config.rs — Route configuration store
//
// Loads provider accounts and model-routing rules from YAML or Redis, then
// exposes fast candidate resolution and model listing.
// ---------------------------------------------------------------------------

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::console::document::{
    canonicalize_route_document, inspect_route_document, materialize_credential_ids,
    validate_route_document, RouteConfigDiagnostics, ValidatedRouteDocument,
};
use crate::console::revision::{RevisionActor, RevisionMetadata};
use crate::credential_runtime::{KeepaliveConfig, SessionAuthConfig};
use crate::preset::{compile_provider_account, get_builtin_preset, AccountOverrides};
use crate::protocol::registry::{
    canonicalize_protocol_family_key, canonicalize_protocol_profile_key,
    default_protocol_profile_for_preset, infer_protocol_family, infer_protocol_profile,
};
use crate::protocol::search_api::resolve_legacy_search_route_model_alias;
use crate::routing::candidate::{
    canonicalize_adapter_name, ProviderAccountPayload, ProviderExecutionMode, RouteCandidate,
};
use crate::routing::protocol_resolution::surface_supported_wire_protocol_families;

// ---------------------------------------------------------------------------
// Public types used by callers
// ---------------------------------------------------------------------------

mod account_constraints;
mod account_inventory;
mod aliases;
mod candidates;
mod default_group;
mod document_compilation;
mod model_catalog;
mod model_mapping;
mod payload;
mod provider_compilation;
mod refresh_state;
mod schema;
mod snapshot;
mod snapshot_publication;
mod storage_connection;
mod store;
mod substitution;
mod test_plans;
mod test_policy;

pub(crate) use aliases::normalized_alias_conflicts;
pub use candidates::CandidateResolution;
pub(crate) use document_compilation::{
    compile_route_document, effective_credential_id, provider_default_account_id,
};
pub(crate) use model_mapping::model_for_probe;
pub(crate) use refresh_state::effective_refresh_lifetime_secs;
pub use refresh_state::{future_rfc3339_after_secs, system_time_to_rfc3339_millis};
pub use schema::{
    AccountGroupYaml, ModelInfo, ModelRoute, ProviderConfigYaml, ProviderCredentialYaml,
    RouteAccountGroupInventory, RouteAccountGroupSelectionError, RouteAccountGroupView,
    RouteAccountProviderView, RouteAccountView, RouteConfigYaml,
};
pub use storage_connection::CredentialStorageConnection;
pub use store::RouteConfigReplaceError;
pub(crate) use substitution::subst_env;

use account_constraints::build_account_group_constraint;
#[cfg(test)]
use account_constraints::candidate_account_group_member_id;
use account_inventory::build_route_account_group_inventory;
use aliases::{glob_match, normalized_alias_entries, resolve_alias_inner};
#[cfg(test)]
use candidates::select_credential;
use candidates::{
    apply_token_override, resolve_candidates_inner, resolve_candidates_inner_with_account_filter,
};
#[cfg(test)]
use document_compilation::compile_yaml;
use document_compilation::compile_yaml_with_fingerprints;
use model_catalog::list_models_inner;
use payload::build_base_payload;
use provider_compilation::compile_provider;
use snapshot_publication::{build_snapshot, validate_revision_matches_document};
use substitution::subst_provider;

/// A compiled provider ready for routing — all preset merging done at load time.
#[derive(Debug, Clone)]
pub struct CompiledProvider {
    pub id: String,
    pub label: String,
    pub payload: ProviderAccountPayload,
    pub protocol_family: String,
    pub protocol_profile: String,
    /// Explicit supported-model list.  Empty means the provider accepts any model.
    pub supported_models: Vec<String>,
    /// Per-provider model name translation.
    /// Key = canonical model name, Value = what this provider actually calls it.
    /// E.g., `{"claude-opus-4-6": "opus4.6"}` if this provider uses a different name.
    pub model_map: HashMap<String, String>,
    pub model_map_targets: HashMap<String, Vec<String>>,
    pub scheduled_probe_enabled: bool,
    pub scheduled_probe_interval_minutes: u64,
    /// Credential pool for this provider.
    /// Empty = single credential baked into `payload`.
    /// Non-empty = select one at request time (round-robin).
    pub credential_pool: Vec<CompiledCredential>,
    /// Atomic counter for round-robin credential selection.
    /// Stored in an `Arc` so that cloning `CompiledProvider` shares the counter.
    pub credential_counter: std::sync::Arc<AtomicUsize>,
}

/// A single credential within a provider's credential pool.
/// Contains a complete `ProviderAccountPayload` with the credential's auth
/// merged in — ready to use with no per-request merging needed.
#[derive(Debug, Clone)]
pub struct CompiledCredential {
    pub discovery_required: bool,
    pub discovery: Option<crate::provider_discovery::CredentialDiscovery>,
    pub id: String,
    pub payload: ProviderAccountPayload,
    /// Whether this credential participates in runtime routing. Disabled
    /// credentials remain compiled so the inventory and management console can
    /// display and re-enable them without losing their identity.
    pub enabled: bool,
    /// Models this credential can serve. Empty = any model the provider supports.
    pub supported_models: Vec<String>,
    pub scheduled_probe_enabled: bool,
    pub scheduled_probe_interval_minutes: u64,
    /// OAuth token refresh config. When present, access_token is auto-refreshed.
    pub refresh_config: Option<Arc<TokenRefreshState>>,
}

/// Holds the mutable state for OAuth token refresh.
pub struct TokenRefreshState {
    pub refresh_token: parking_lot::Mutex<String>,
    pub refresh_endpoint: String,
    pub client_id: String,
    pub expires_in_secs: u64,
    pub expires_at: parking_lot::Mutex<std::time::Instant>,
    pub expires_at_iso: parking_lot::Mutex<Option<String>>,
    /// Overrides payload.api_key when a refreshed token is available.
    pub api_key_override: parking_lot::Mutex<Option<String>>,
    lifecycle: parking_lot::Mutex<RefreshLifecycle>,
}

#[derive(Debug)]
struct RefreshLifecycle {
    active: bool,
    generation: u64,
}

pub(crate) fn safe_refresh_deadline(seconds: u64) -> std::time::Instant {
    let now = std::time::Instant::now();
    let duration = std::time::Duration::from_secs(effective_refresh_lifetime_secs(seconds));
    now.checked_add(duration).unwrap_or(now)
}

/// Validated request-time account-group membership constraint.
///
/// Candidate sources use different storage backends, but all of them resolve
/// to a concrete `RouteCandidate`. Applying one constraint to that common type
/// makes group isolation independent of whether a candidate came from the
/// access catalog, Redis, PostgreSQL routing, or the YAML document.
#[derive(Debug, Clone)]
pub struct RouteAccountGroupConstraint {
    requested_group_id: Option<String>,
    allowed_account_ids: Option<HashSet<String>>,
}

pub(crate) struct RouteConfigInner {
    providers: Vec<CompiledProvider>,
    model_routes: Vec<ModelRoute>,
    /// Exact alias map: "opus" → "claude-opus-4-6"
    aliases: HashMap<String, String>,
    /// Pre-computed normalized alias map (built once at load time).
    /// Key = normalized alias key (no `-._`, lowercase), Value = canonical model name.
    /// Eliminates per-request string allocation during fuzzy matching.
    normalized_aliases: HashMap<String, String>,
    /// Pre-computed sorted normalized keys for prefix matching (built once).
    /// Each entry: (normalized_key, canonical_model_name).
    sorted_normalized_keys: Vec<(String, String)>,
}

/// Where the active route document was obtained from.
///
/// This is deliberately metadata only.  The snapshot's document and compiled
/// providers remain private so callers cannot accidentally serialize secrets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveConfigSource {
    Yaml,
    Redis,
    Database,
    Recovered,
}

/// One immutable, internally consistent route configuration view.
///
/// Every request-path query pins one `Arc<RouteConfigSnapshot>` and then reads
/// all of its fields.  A replacement publishes a wholly-built snapshot in one
/// pointer swap, so readers can never observe a mixed document/compiler state.
pub struct RouteConfigSnapshot {
    revision: RevisionMetadata,
    source: ActiveConfigSource,
    #[allow(dead_code)] // Consumed by the transaction runtime added in Task 5.
    document: RouteConfigYaml,
    effective_account_groups: Vec<AccountGroupYaml>,
    diagnostics: RouteConfigDiagnostics,
    compiled: Arc<RouteConfigInner>,
    provider_fingerprints: HashMap<String, String>,
    account_group_inventory_cache: OnceLock<RouteAccountGroupInventory>,
}

#[derive(Debug, Clone)]
pub(crate) struct CredentialProbeTarget {
    pub credential_id: String,
    pub provider_id: String,
    pub enabled: bool,
    pub payload: ProviderAccountPayload,
}

#[derive(Debug, Clone)]
pub(crate) struct ScheduledCredentialProbeTarget {
    pub target: CredentialProbeTarget,
    pub interval_minutes: u64,
    pub plan_id: Option<String>,
}

/// Thread-safe route config store.  Wrap in [`Arc`] and share across threads.
pub struct RouteConfigStore {
    inner: ArcSwap<RouteConfigSnapshot>,
    replace_lock: parking_lot::Mutex<()>,
}

/// Normalize a model name for fuzzy alias matching.
/// Strips `-`, `.`, `_`, and lowercases.
/// E.g., `"claude-opus-4-6"` → `"claudeopus46"`,
///       `"opus4.6"` → `"opus46"`,
///       `"Opus4-6"` → `"opus46"`.
pub(crate) fn normalize_model_name(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '-' && *c != '.' && *c != '_')
        .flat_map(|c| c.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests;
