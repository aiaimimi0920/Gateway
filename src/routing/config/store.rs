//! Route-store construction and serialized atomic snapshot replacement.

use super::*;

#[derive(Debug, thiserror::Error, Clone, Eq, PartialEq)]
pub enum RouteConfigReplaceError {
    #[error("revision metadata is invalid: {0}")]
    InvalidRevision(String),
    #[error("revision document or YAML digest does not match the validated document")]
    RevisionDigestMismatch,
    #[error("validated document canonical bytes do not match its digests")]
    ValidatedDocumentMismatch,
    #[error("active route revision is unchanged")]
    RevisionUnchanged,
    #[error("replacement revision must be the direct child of the active revision")]
    RevisionConflict,
    #[error("external snapshot installs only support Redis or recovered revisions")]
    ExternalSourceUnsupported,
    #[error("strict route validation failed: {0}")]
    Validation(String),
    #[error("route document compilation failed: {0}")]
    Compilation(String),
}

impl RouteConfigReplaceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRevision(_) => "console_revision_invalid",
            Self::RevisionDigestMismatch => "console_revision_digest_mismatch",
            Self::ValidatedDocumentMismatch => "console_validated_document_mismatch",
            Self::RevisionUnchanged => "console_revision_unchanged",
            Self::RevisionConflict => "console_revision_conflict",
            Self::ExternalSourceUnsupported => "console_revision_source_invalid",
            Self::Validation(_) => "console_route_validation_failed",
            Self::Compilation(_) => "console_route_compilation_failed",
        }
    }
}

impl RouteConfigStore {
    // ── Constructors ─────────────────────────────────────────────────────────

    /// Create an empty store (no providers, no routes).
    pub fn new() -> Self {
        let document = RouteConfigYaml {
            providers: Vec::new(),
            model_routes: Vec::new(),
            aliases: HashMap::new(),
            account_groups: Vec::new(),
        };
        Self::from_unchecked_document(
            document,
            ActiveConfigSource::Database,
            RevisionActor::Bootstrap,
            0,
            None,
            None,
        )
        .expect("empty route document must compile")
    }

    /// Strictly validate and construct a store from a route document.
    pub fn from_document(document: RouteConfigYaml) -> Result<Self, anyhow::Error> {
        let validated = validate_route_document(document).map_err(|diagnostics| {
            anyhow::anyhow!("strict route validation failed: {diagnostics}")
        })?;
        let revision = RevisionMetadata::from_validated(
            0,
            None,
            RevisionActor::Bootstrap,
            OffsetDateTime::now_utc(),
            None,
            &validated,
        );
        let (compiled, fingerprints) =
            compile_yaml_with_fingerprints(validated.document().clone())?;
        Ok(Self {
            inner: ArcSwap::from_pointee(RouteConfigSnapshot::new(
                revision,
                ActiveConfigSource::Database,
                validated.document().clone(),
                validated.diagnostics().clone(),
                compiled,
                fingerprints,
            )),
            replace_lock: parking_lot::Mutex::new(()),
        })
    }

    /// Load from a YAML file at `path`.
    ///
    /// Returns `Err` if the file cannot be read or parsed, or if any provider
    /// references an unknown preset.
    pub fn load_from_yaml(path: impl AsRef<std::path::Path>) -> Result<Self, anyhow::Error> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Cannot read route config '{}': {}", path.display(), e))?;
        let config: RouteConfigYaml = serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("YAML parse error in '{}': {}", path.display(), e))?;
        Self::from_unchecked_document(
            config,
            ActiveConfigSource::Yaml,
            RevisionActor::Bootstrap,
            0,
            None,
            None,
        )
    }

    /// Load from Redis (key `gw:config:routes`), where the value is a
    /// JSON-encoded `RouteConfigYaml`.
    pub async fn load_from_redis(pool: &deadpool_redis::Pool) -> Result<Self, anyhow::Error> {
        use redis::AsyncCommands;

        let mut conn = pool
            .get()
            .await
            .map_err(|e| anyhow::anyhow!("Redis pool error: {}", e))?;

        let raw: String = conn
            .get("gw:config:routes")
            .await
            .map_err(|e| anyhow::anyhow!("Redis GET gw:config:routes: {}", e))?;

        let config: RouteConfigYaml = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("JSON parse error from Redis: {}", e))?;

        Self::from_unchecked_document(
            config,
            ActiveConfigSource::Redis,
            RevisionActor::Bootstrap,
            0,
            None,
            None,
        )
    }

    pub(super) fn from_unchecked_document(
        document: RouteConfigYaml,
        source: ActiveConfigSource,
        actor: RevisionActor,
        sequence: u64,
        parent: Option<String>,
        message: Option<String>,
    ) -> Result<Self, anyhow::Error> {
        let diagnostics = inspect_route_document(&document).diagnostics;
        let canonical = canonicalize_route_document(&document)?;
        let revision = RevisionMetadata::from_canonical(
            sequence,
            parent,
            actor,
            OffsetDateTime::now_utc(),
            message,
            &canonical,
        );
        let (compiled, fingerprints) = compile_yaml_with_fingerprints(document.clone())?;
        Ok(Self {
            inner: ArcSwap::from_pointee(RouteConfigSnapshot::new(
                revision,
                source,
                document,
                diagnostics,
                compiled,
                fingerprints,
            )),
            replace_lock: parking_lot::Mutex::new(()),
        })
    }

    /// Pin the currently active immutable snapshot.
    pub fn snapshot(&self) -> Arc<RouteConfigSnapshot> {
        self.inner.load_full()
    }

    /// Strictly replace the active document and derive a new local revision.
    pub fn replace_document(
        &self,
        document: RouteConfigYaml,
    ) -> Result<Arc<RouteConfigSnapshot>, RouteConfigReplaceError> {
        let validated = validate_route_document(document)
            .map_err(|diagnostics| RouteConfigReplaceError::Validation(diagnostics.to_string()))?;
        let current = self.snapshot();
        let revision = RevisionMetadata::from_validated(
            current.revision().sequence().saturating_add(1),
            Some(current.revision().id().to_string()),
            RevisionActor::EnvironmentOverride,
            OffsetDateTime::now_utc(),
            None,
            &validated,
        );
        self.replace_validated(validated, revision)
    }

    /// Atomically publish a strictly validated document/revision pair.
    pub fn replace_validated(
        &self,
        validated: ValidatedRouteDocument,
        revision: RevisionMetadata,
    ) -> Result<Arc<RouteConfigSnapshot>, RouteConfigReplaceError> {
        let _replace_guard = self.replace_lock.lock();
        let current = self.inner.load_full();
        validate_revision_matches_document(&revision, &validated)?;
        // Replicas may converge directly from an older active revision to a
        // later Redis revision, so monotonic gaps are valid. The parent must
        // still identify the exact snapshot being replaced.
        if revision.sequence() <= current.revision().sequence()
            || revision.parent() != Some(current.revision().id())
        {
            return Err(RouteConfigReplaceError::RevisionConflict);
        }
        if revision.document_digest() == current.revision().document_digest()
            && revision.yaml_digest() == current.revision().yaml_digest()
        {
            return Err(RouteConfigReplaceError::RevisionUnchanged);
        }

        let snapshot = build_snapshot(validated, revision, current.source(), Some(&current))?;
        self.inner.store(snapshot.clone());
        Ok(snapshot)
    }

    pub fn from_external_validated(
        validated: ValidatedRouteDocument,
        revision: RevisionMetadata,
        source: ActiveConfigSource,
    ) -> Result<Self, RouteConfigReplaceError> {
        if !matches!(
            source,
            ActiveConfigSource::Redis | ActiveConfigSource::Recovered
        ) {
            return Err(RouteConfigReplaceError::ExternalSourceUnsupported);
        }
        let snapshot = build_snapshot(validated, revision, source, None)?;
        Ok(Self {
            inner: ArcSwap::from(snapshot),
            replace_lock: parking_lot::Mutex::new(()),
        })
    }

    pub fn install_external_validated(
        &self,
        validated: ValidatedRouteDocument,
        revision: RevisionMetadata,
        source: ActiveConfigSource,
    ) -> Result<Arc<RouteConfigSnapshot>, RouteConfigReplaceError> {
        if !matches!(
            source,
            ActiveConfigSource::Redis | ActiveConfigSource::Recovered
        ) {
            return Err(RouteConfigReplaceError::ExternalSourceUnsupported);
        }
        let _replace_guard = self.replace_lock.lock();
        let current = self.inner.load_full();
        validate_revision_matches_document(&revision, &validated)?;
        if revision.sequence() <= current.revision().sequence() {
            return Err(RouteConfigReplaceError::RevisionConflict);
        }
        let snapshot = build_snapshot(validated, revision, source, Some(&current))?;
        self.inner.store(snapshot.clone());
        Ok(snapshot)
    }

    // ── Query API ─────────────────────────────────────────────────────────────

    /// Resolve the canonical model name through the alias map with smart
    /// fuzzy matching.
    ///
    /// Resolution strategy (in order):
    /// 1. Exact match: `"opus"` → aliases["opus"] — O(1) HashMap lookup
    /// 2. Normalized match: pre-computed normalized_aliases lookup — O(1)
    /// 3. Prefix match: binary search on sorted_normalized_keys — O(log N)
    ///
    /// **Zero heap allocation per request** — all normalization is done at
    /// config load time. The only allocation is the cloned result String.
    pub fn resolve_alias(&self, model: Option<&str>) -> Option<String> {
        self.snapshot().resolve_alias(model)
    }

    /// Resolve the ordered list of [`RouteCandidate`]s for `model`.
    ///
    /// Resolution algorithm:
    /// 1. Resolve the model name through the alias map.
    /// 2. Find all `model_routes` whose pattern matches the resolved model
    ///    (glob match).
    /// 3. Sort matching routes by `priority` descending (highest first).
    /// 4. Collect the provider IDs from those routes in priority order,
    ///    deduplicating while preserving order.
    /// 5. Map each provider ID to a `RouteCandidate`.
    /// 6. If no routes matched, prefer providers whose non-empty
    ///    `supported_models` exactly contains the resolved model.
    /// 7. If no provider explicitly supports the model, fall back to ALL
    ///    configured providers.
    /// 8. If `model` is `None`, return ALL providers.
    pub fn resolve_candidates(&self, model: Option<&str>) -> Vec<RouteCandidate> {
        self.snapshot().resolve_candidates(model)
    }

    pub fn resolve_candidates_for_account_group(
        &self,
        model: Option<&str>,
        account_group_id: Option<&str>,
    ) -> Result<Vec<RouteCandidate>, RouteAccountGroupSelectionError> {
        self.snapshot()
            .resolve_candidates_for_account_group(model, account_group_id)
    }

    pub fn account_group_constraint(
        &self,
        account_group_id: Option<&str>,
    ) -> Result<RouteAccountGroupConstraint, RouteAccountGroupSelectionError> {
        self.snapshot().account_group_constraint(account_group_id)
    }

    /// List all models available through this gateway (for `GET /v1/models`).
    ///
    /// Derived from:
    /// - Explicit `supported_models` on each provider.
    /// - Patterns from `model_routes` that are exact names (no `*`).
    /// - Alias keys (the short name) and their resolved targets.
    pub fn list_models(&self) -> Vec<ModelInfo> {
        self.snapshot().list_models()
    }

    /// Returns `true` if at least one provider is configured.
    pub fn has_routes(&self) -> bool {
        self.snapshot().has_routes()
    }

    /// Number of configured providers.
    pub fn provider_count(&self) -> usize {
        self.snapshot().provider_count()
    }

    /// Get a snapshot of all providers (for token refresh task).
    pub fn get_providers(&self) -> Vec<CompiledProvider> {
        self.snapshot().get_providers()
    }

    pub fn account_group_inventory(&self) -> RouteAccountGroupInventory {
        self.snapshot().account_group_inventory()
    }
}

impl Default for RouteConfigStore {
    fn default() -> Self {
        Self::new()
    }
}
