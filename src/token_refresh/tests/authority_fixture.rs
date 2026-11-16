//! Normal Server coordinator with a strict in-memory active-revision CAS, not a Redis engine.
use super::*;
use crate::console::{
    RouteConfigRedisActivationOutcome, RouteConfigRedisBackend, RouteConfigRedisRevision,
    RouteConfigRedisStoreError, TransactionRecord,
};
use std::sync::Mutex;
#[path = "grant_backend.rs"]
mod grants;

#[derive(Clone, Copy)]
pub(super) enum Stall {
    Load = 1,
    BeforeActivation = 2,
    AfterActivation = 3,
    AfterGrantReservation = 4,
    AfterGrantResult = 5,
    AfterGrantPublication = 6,
    AfterGrantReceipt = 7,
}

#[derive(Clone)]
pub(super) struct Backend {
    state: Arc<Mutex<AuthorityState>>,
    stall: Arc<AtomicUsize>,
    reached: Arc<tokio::sync::Notify>,
    activations: Arc<AtomicUsize>,
    settlements: Arc<AtomicUsize>,
    absent: Arc<std::sync::atomic::AtomicBool>,
    cache: Arc<Mutex<Option<deadpool_redis::Pool>>>,
}
struct AuthorityState {
    active: RouteConfigRedisRevision,
    transactions: std::collections::BTreeMap<String, TransactionRecord>,
    grants:
        std::collections::BTreeMap<(String, String), crate::console::refresh_grants::GrantRecord>,
    grant_heads: std::collections::BTreeMap<String, String>,
}
impl Backend {
    pub fn grant_record(
        &self,
        request: &crate::console::refresh_grants::GrantRequest,
    ) -> Option<crate::console::refresh_grants::GrantRecord> {
        self.state
            .lock()
            .unwrap()
            .grants
            .get(&(request.family_id(), request.material_id()))
            .cloned()
    }
    pub fn bind_cache(&self, pool: &deadpool_redis::Pool) {
        *self.cache.lock().unwrap() = Some(pool.clone());
    }
    pub fn stall(&self, stage: Stall) {
        self.stall.store(stage as usize, Ordering::SeqCst);
    }
    pub fn resume(&self) {
        // The old never-ready future must have been dropped, not resumed in a detached task.
        self.stall.store(0, Ordering::SeqCst);
    }
    pub async fn wait_stalled(&self) {
        tokio::time::timeout(Duration::from_secs(3), self.reached.notified())
            .await
            .expect("authority did not reach the selected stall");
    }
    pub fn activations(&self) -> usize {
        self.activations.load(Ordering::SeqCst)
    }
    pub fn settlements(&self) -> usize {
        self.settlements.load(Ordering::SeqCst)
    }
    pub fn set_absent(&self) {
        self.absent.store(true, Ordering::SeqCst);
    }
    async fn pause(&self, stage: Stall) {
        if self.stall.load(Ordering::SeqCst) == stage as usize {
            self.reached.notify_one();
            std::future::pending::<()>().await;
        }
    }
}

#[async_trait::async_trait]
impl RouteConfigRedisBackend for Backend {
    async fn load_refresh_grant(
        &self,
        locator: &crate::console::refresh_grants::GrantLocator,
    ) -> Result<Option<crate::console::refresh_grants::GrantRecord>, RouteConfigRedisStoreError>
    {
        self.gate_load(locator)
    }
    async fn repair_refresh_grant_cache(
        &self,
        revision: &RouteConfigRedisRevision,
        record: &crate::console::refresh_grants::GrantRecord,
        transaction: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.gate_repair(revision, record, transaction).await
    }
    async fn reserve_refresh_grant(
        &self,
        revision: &RouteConfigRedisRevision,
        request: &crate::console::refresh_grants::GrantRequest,
    ) -> Result<crate::console::refresh_grants::GrantReservation, RouteConfigRedisStoreError> {
        let result = self.gate_reserve(revision, request);
        self.pause(Stall::AfterGrantReservation).await;
        result
    }
    async fn store_refresh_grant_result(
        &self,
        record: &crate::console::refresh_grants::GrantRecord,
        result: &crate::console::refresh_grants::GrantResult,
    ) -> Result<crate::console::refresh_grants::GrantRecord, RouteConfigRedisStoreError> {
        let result = self.gate_result(record, result);
        self.pause(Stall::AfterGrantResult).await;
        result
    }
    async fn store_refresh_grant_receipt(
        &self,
        record: &crate::console::refresh_grants::GrantRecord,
        receipt: &crate::console::refresh_grants::GrantReceipt,
    ) -> Result<crate::console::refresh_grants::GrantRecord, RouteConfigRedisStoreError> {
        let result = self.gate_receipt(record, receipt);
        self.pause(Stall::AfterGrantReceipt).await;
        result
    }
    async fn mark_refresh_grant_published(
        &self,
        revision: &RouteConfigRedisRevision,
        record: &crate::console::refresh_grants::GrantRecord,
    ) -> Result<crate::console::refresh_grants::GrantRecord, RouteConfigRedisStoreError> {
        let result = self.gate_publish(revision, record);
        self.pause(Stall::AfterGrantPublication).await;
        result
    }
    async fn settle_transaction_locked(
        &self,
        _: &crate::console::WriterLockGuard,
        record: &TransactionRecord,
    ) -> Result<crate::console::RouteConfigRedisSettlement, RouteConfigRedisStoreError> {
        use crate::console::{RouteConfigRedisSettlement as Settled, TransactionPhase};
        self.settlements.fetch_add(1, Ordering::SeqCst);
        let mut state = self.state.lock().unwrap();
        let tombstone = record.revocation_tombstone().unwrap();
        if let Some(stored) = state.transactions.get(record.tx_id()) {
            if stored == &tombstone {
                return Ok(Settled::Revoked);
            }
            assert_eq!(
                stored.prepared_identity().unwrap(),
                record.prepared_identity().unwrap()
            );
            if stored.phase() == TransactionPhase::RedisActivated {
                return Ok(Settled::Activated);
            }
            assert_eq!(stored.phase(), TransactionPhase::Prepared);
        }
        state.transactions.insert(record.tx_id().into(), tombstone);
        Ok(Settled::Revoked)
    }

    async fn store_revision(
        &self,
        _: &RouteConfigRedisRevision,
    ) -> Result<(), RouteConfigRedisStoreError> {
        Ok(())
    }
    async fn store_prepared_transaction(
        &self,
        record: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        let mut state = self.state.lock().unwrap();
        if let Some(old) = state.transactions.get(record.tx_id()) {
            assert_eq!(old, record);
        }
        state
            .transactions
            .insert(record.tx_id().into(), record.clone());
        Ok(())
    }
    async fn activate_revision(
        &self,
        expected: Option<&str>,
        next: &RouteConfigRedisRevision,
        prepared: &TransactionRecord,
        activated: &TransactionRecord,
    ) -> Result<RouteConfigRedisActivationOutcome, RouteConfigRedisStoreError> {
        self.pause(Stall::BeforeActivation).await;
        {
            let mut state = self.state.lock().unwrap();
            assert_eq!(state.transactions.get(prepared.tx_id()), Some(prepared));
            if expected != Some(state.active.metadata().id()) {
                return Ok(RouteConfigRedisActivationOutcome::RevisionConflict {
                    actual: Some(state.active.metadata().id().into()),
                });
            }
            state.active = next.clone();
            state
                .transactions
                .insert(prepared.tx_id().into(), activated.clone());
            self.activations.fetch_add(1, Ordering::SeqCst);
        }
        self.pause(Stall::AfterActivation).await;
        Ok(RouteConfigRedisActivationOutcome::Activated)
    }
    async fn load_active_revision(
        &self,
    ) -> Result<Option<RouteConfigRedisRevision>, RouteConfigRedisStoreError> {
        self.pause(Stall::Load).await;
        if self.absent.load(Ordering::SeqCst) {
            return Ok(None);
        }
        Ok(Some(self.state.lock().unwrap().active.clone()))
    }
}

pub(super) struct Authority {
    pub runtime: RouteConfigRuntime,
    pub persistence: RouteConfigPersistence,
    pub backend: Backend,
    pub routes: PathBuf,
    initial: RouteConfigYaml,
}
impl Authority {
    pub fn new() -> Self {
        Self::with_writable(true)
    }
    pub fn with_writable(writable: bool) -> Self {
        let initial: RouteConfigYaml = serde_json::from_value(grok_context::document()).unwrap();
        Self::with_document(writable, initial)
    }
    pub fn with_document(writable: bool, initial: RouteConfigYaml) -> Self {
        let store = Arc::new(RouteConfigStore::from_document(initial.clone()).unwrap());
        let root = std::env::temp_dir().join(format!(
            "gateway-server-oauth-authority-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let routes = root.join("routes.yaml");
        std::fs::write(&routes, serde_yaml::to_string(&initial).unwrap()).unwrap();
        let persistence = RouteConfigPersistence::for_test(&root, &routes).unwrap();
        let snapshot = store.snapshot();
        let backend = Backend {
            state: Arc::new(Mutex::new(AuthorityState {
                active: RouteConfigRedisRevision::new(
                    snapshot.revision().clone(),
                    snapshot.document().clone(),
                )
                .unwrap(),
                transactions: Default::default(),
                grants: Default::default(),
                grant_heads: Default::default(),
            })),
            stall: Arc::default(),
            reached: Arc::default(),
            activations: Arc::default(),
            settlements: Arc::default(),
            absent: Arc::default(),
            cache: Arc::default(),
        };
        let runtime = RouteConfigRuntime::with_backend(
            store,
            persistence.clone(),
            Arc::new(backend.clone()),
            writable,
        );
        Self {
            runtime,
            persistence,
            backend,
            routes,
            initial,
        }
    }
    pub fn saved(&self) -> crate::routing::config::ProviderCredentialYaml {
        self.runtime.route_config().snapshot().document().providers[0].credentials[0].clone()
    }
    pub fn active(&self) -> RouteConfigRedisRevision {
        self.backend.state.lock().unwrap().active.clone()
    }
    pub async fn restart(&self) -> RouteConfigRuntime {
        let runtime = self.rebuilt(true);
        runtime.recover_startup().await.unwrap();
        runtime
    }
    pub fn rebuilt(&self, writable: bool) -> RouteConfigRuntime {
        // Deliberately start with stale bootstrap material; recovery must read authority, not this mirror.
        let store = Arc::new(RouteConfigStore::from_document(self.initial.clone()).unwrap());
        RouteConfigRuntime::with_backend(
            store,
            self.persistence.clone(),
            Arc::new(self.backend.clone()),
            writable,
        )
    }
}
