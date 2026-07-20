// ---------------------------------------------------------------------------
// ConcurrencyRegistry — per-provider AIMD controller map
//
// Uses DashMap for lock-free concurrent reads and lazy-creation of controllers.
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;

use super::aimd::{AimdConfig, AimdController, ConcurrencySnapshot};

// ---------------------------------------------------------------------------
// ConcurrencyRegistry
// ---------------------------------------------------------------------------

/// A thread-safe registry that lazily creates one [`AimdController`] per
/// provider ID (typically a string like `"openai"` or `"anthropic"`).
pub struct ConcurrencyRegistry {
    controllers: DashMap<String, Arc<AimdController>>,
    default_config: AimdConfig,
}

impl ConcurrencyRegistry {
    /// Create a new registry with the given default config. Every newly created
    /// controller will be initialised with a clone of this config.
    pub fn new(default_config: AimdConfig) -> Self {
        Self {
            controllers: DashMap::new(),
            default_config,
        }
    }

    /// Return the controller for `provider_id`, creating one with the default
    /// config if it does not yet exist.
    ///
    /// The implementation uses the DashMap entry API to avoid TOCTOU races;
    /// at most one controller is ever constructed per provider key.
    pub fn get_or_create(&self, provider_id: &str) -> Arc<AimdController> {
        // Fast path — already exists.
        if let Some(ctrl) = self.controllers.get(provider_id) {
            return Arc::clone(&ctrl);
        }

        // Slow path — insert under the entry lock to avoid double-create.
        let ctrl = Arc::new(AimdController::new(self.default_config.clone()));
        self.controllers
            .entry(provider_id.to_string())
            .or_insert_with(|| ctrl)
            .clone()
    }

    /// Snapshot the current state of every registered controller.
    pub fn snapshot_all(&self) -> HashMap<String, ConcurrencySnapshot> {
        self.controllers
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().snapshot()))
            .collect()
    }

    /// Number of providers currently tracked.
    pub fn len(&self) -> usize {
        self.controllers.len()
    }

    /// Returns true if no providers are registered yet.
    pub fn is_empty(&self) -> bool {
        self.controllers.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> AimdConfig {
        AimdConfig {
            initial_limit: 10,
            min_limit: 1,
            max_limit: 50,
            increase_threshold: 5,
            decrease_factor: 0.7,
            severe_decrease_factor: 0.5,
        }
    }

    #[test]
    fn get_or_create_returns_same_instance() {
        let registry = ConcurrencyRegistry::new(default_config());
        let a = registry.get_or_create("openai");
        let b = registry.get_or_create("openai");
        // Both arcs must point to the same allocation.
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn different_providers_get_different_controllers() {
        let registry = ConcurrencyRegistry::new(default_config());
        let a = registry.get_or_create("openai");
        let b = registry.get_or_create("anthropic");
        assert!(!Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn len_tracks_distinct_providers() {
        let registry = ConcurrencyRegistry::new(default_config());
        assert_eq!(registry.len(), 0);
        registry.get_or_create("openai");
        registry.get_or_create("anthropic");
        registry.get_or_create("openai"); // duplicate — should not increment
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn snapshot_all_covers_all_registered_providers() {
        let registry = ConcurrencyRegistry::new(default_config());
        registry.get_or_create("openai");
        registry.get_or_create("anthropic");

        let snapshots = registry.snapshot_all();
        assert!(snapshots.contains_key("openai"));
        assert!(snapshots.contains_key("anthropic"));
        assert_eq!(snapshots.len(), 2);
    }

    #[test]
    fn snapshot_reflects_controller_state() {
        let registry = ConcurrencyRegistry::new(default_config());
        let ctrl = registry.get_or_create("groq");
        ctrl.on_failure(super::super::aimd::FailureKind::RateLimited);

        let snapshots = registry.snapshot_all();
        let snap = snapshots.get("groq").expect("groq snapshot");
        // floor(10 * 0.5) = 5
        assert_eq!(snap.current_limit, 5);
    }

    #[test]
    fn registry_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ConcurrencyRegistry>();
    }
}
