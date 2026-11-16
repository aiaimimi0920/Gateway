//! One explicitly owned command can outlive its caller; tests release and join it deterministically.
use super::*;
use crate::console::document::canonicalize_route_document;
use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};
use tokio::sync::{oneshot, Notify, Semaphore};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Stage {
    Prepare,
    Activate,
}
#[derive(Clone)]
enum Command {
    Prepare(TransactionRecord),
    Activate(
        Option<String>,
        RouteConfigRedisRevision,
        TransactionRecord,
        TransactionRecord,
    ),
}
impl Command {
    fn stage(&self) -> Stage {
        match self {
            Self::Prepare(_) => Stage::Prepare,
            Self::Activate(..) => Stage::Activate,
        }
    }
}
#[derive(Clone, Debug)]
pub(super) enum Reply {
    Prepared(bool),
    Activated(RouteConfigRedisActivationOutcome),
}
struct State {
    active: RouteConfigRedisRevision,
    revisions: BTreeMap<String, RouteConfigRedisRevision>,
    transactions: BTreeMap<String, TransactionRecord>,
}
#[derive(Clone)]
pub(super) struct Backend {
    state: Arc<Mutex<State>>,
    stage: Stage,
    queued: Arc<std::sync::atomic::AtomicBool>,
    accepted: Arc<Notify>,
    gate: Arc<Semaphore>,
    task: Arc<Mutex<Option<tokio::task::JoinHandle<Reply>>>>,
    after_settle: Arc<Mutex<Option<RouteConfigRedisRevision>>>,
    lose_settle_reply: Arc<std::sync::atomic::AtomicBool>,
}
impl Backend {
    fn execute(state: &mut State, command: Command) -> Reply {
        match command {
            Command::Prepare(record) => {
                if state
                    .transactions
                    .get(record.tx_id())
                    .is_some_and(|old| old != &record)
                {
                    return Reply::Prepared(false);
                }
                state.transactions.insert(record.tx_id().into(), record);
                Reply::Prepared(true)
            }
            Command::Activate(expected, revision, prepared, activated) => {
                let outcome = if state.transactions.get(prepared.tx_id()) != Some(&prepared) {
                    RouteConfigRedisActivationOutcome::PreparedTransactionMismatch
                } else if state.active.metadata().id() != expected.as_deref().unwrap_or("") {
                    RouteConfigRedisActivationOutcome::RevisionConflict {
                        actual: Some(state.active.metadata().id().into()),
                    }
                } else if state
                    .revisions
                    .get(revision.metadata().id())
                    .map(|r| r.to_json().unwrap())
                    != Some(revision.to_json().unwrap())
                {
                    RouteConfigRedisActivationOutcome::RevisionPayloadMismatch
                } else {
                    state
                        .transactions
                        .insert(prepared.tx_id().into(), activated);
                    state.active = revision;
                    RouteConfigRedisActivationOutcome::Activated
                };
                Reply::Activated(outcome)
            }
        }
    }
    async fn submit(&self, command: Command) -> Reply {
        if command.stage() != self.stage
            || !self.queued.swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Self::execute(&mut self.state.lock().unwrap(), command);
        }
        let (sender, receiver) = oneshot::channel();
        let (state, gate, accepted) =
            (self.state.clone(), self.gate.clone(), self.accepted.clone());
        let task = tokio::spawn(async move {
            accepted.notify_one();
            gate.acquire_owned().await.unwrap().forget();
            let reply = Self::execute(&mut state.lock().unwrap(), command);
            // Losing the response receiver does not cancel an already accepted command.
            let _ = sender.send(reply.clone());
            reply
        });
        assert!(
            self.task.lock().unwrap().replace(task).is_none(),
            "only one queued command per fixture"
        );
        receiver.await.unwrap()
    }
    pub async fn wait_accepted(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(3), self.accepted.notified())
            .await
            .unwrap();
    }
    pub async fn release_and_join(&self) -> Reply {
        let task = self.task.lock().unwrap().take().unwrap();
        self.gate.add_permits(1);
        tokio::time::timeout(std::time::Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap()
    }
    pub fn active(&self) -> RouteConfigRedisRevision {
        self.state.lock().unwrap().active.clone()
    }
    pub fn transaction(&self, id: &str) -> TransactionRecord {
        self.state.lock().unwrap().transactions[id].clone()
    }
    pub fn candidate(&self, id: &str) -> RouteConfigRedisRevision {
        self.state.lock().unwrap().revisions[id].clone()
    }
    pub fn set_transaction(&self, record: TransactionRecord) {
        self.state
            .lock()
            .unwrap()
            .transactions
            .insert(record.tx_id().into(), record);
    }
    pub fn set_active(&self, revision: RouteConfigRedisRevision) {
        self.state.lock().unwrap().active = revision;
    }
    pub fn advance_after_settle(&self, revision: RouteConfigRedisRevision) {
        *self.after_settle.lock().unwrap() = Some(revision);
    }
    pub fn lose_settle_response(&self) {
        self.lose_settle_reply
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
#[async_trait]
impl RouteConfigRedisBackend for Backend {
    async fn settle_transaction_locked(
        &self,
        _: &crate::console::WriterLockGuard,
        record: &TransactionRecord,
    ) -> Result<RouteConfigRedisSettlement, RouteConfigRedisStoreError> {
        let outcome = {
            let mut state = self.state.lock().unwrap();
            let prepared = record.prepared_identity().unwrap();
            let tombstone = record.revocation_tombstone().unwrap();
            let mut outcome = RouteConfigRedisSettlement::Revoked;
            if let Some(stored) = state.transactions.get(record.tx_id()) {
                if stored.prepared_identity().unwrap() != prepared {
                    return Err(RouteConfigRedisStoreError::from_persistence(
                        PersistenceError::recovery_required("Synthetic gate mismatch"),
                    ));
                }
                if stored.phase() == crate::console::TransactionPhase::RedisActivated {
                    outcome = RouteConfigRedisSettlement::Activated;
                } else {
                    assert!(stored == &prepared || stored == &tombstone);
                }
            }
            if outcome == RouteConfigRedisSettlement::Revoked {
                state.transactions.insert(record.tx_id().into(), tombstone);
            }
            if let Some(next) = self.after_settle.lock().unwrap().take() {
                state.active = next;
            }
            outcome
        };
        if self
            .lose_settle_reply
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            std::future::pending::<()>().await;
        }
        Ok(outcome)
    }

    async fn store_revision(
        &self,
        revision: &RouteConfigRedisRevision,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.state
            .lock()
            .unwrap()
            .revisions
            .insert(revision.metadata().id().into(), revision.clone());
        Ok(())
    }
    async fn store_prepared_transaction(
        &self,
        record: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        match self.submit(Command::Prepare(record.clone())).await {
            Reply::Prepared(true) => Ok(()),
            Reply::Prepared(false) => Err(RouteConfigRedisStoreError::from_persistence(
                PersistenceError::recovery_required("Synthetic immutable prepared mismatch"),
            )),
            _ => unreachable!("wrong reply kind"),
        }
    }
    async fn activate_revision(
        &self,
        expected: Option<&str>,
        revision: &RouteConfigRedisRevision,
        prepared: &TransactionRecord,
        activated: &TransactionRecord,
    ) -> Result<RouteConfigRedisActivationOutcome, RouteConfigRedisStoreError> {
        match self
            .submit(Command::Activate(
                expected.map(str::to_owned),
                revision.clone(),
                prepared.clone(),
                activated.clone(),
            ))
            .await
        {
            Reply::Activated(outcome) => Ok(outcome),
            _ => unreachable!("wrong reply kind"),
        }
    }
    async fn load_active_revision(
        &self,
    ) -> Result<Option<RouteConfigRedisRevision>, RouteConfigRedisStoreError> {
        Ok(Some(self.active()))
    }
}

pub(super) struct Harness {
    pub runtime: RouteConfigRuntime,
    pub backend: Backend,
    pub persistence: RouteConfigPersistence,
    pub journal: TransactionJournal,
    pub routes: PathBuf,
    pub original_yaml: Vec<u8>,
}
impl Harness {
    pub fn new(stage: Stage) -> Self {
        let root =
            std::env::temp_dir().join(format!("gateway-late-transaction-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let routes = root.join("routes.yaml");
        let document = document("original");
        let original_yaml = canonicalize_route_document(&document)
            .unwrap()
            .canonical_yaml()
            .to_vec();
        std::fs::write(&routes, &original_yaml).unwrap();
        let store = Arc::new(RouteConfigStore::from_document(document).unwrap());
        let snapshot = store.snapshot();
        let active =
            RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
                .unwrap();
        let backend = Backend {
            state: Arc::new(Mutex::new(State {
                active,
                revisions: BTreeMap::new(),
                transactions: BTreeMap::new(),
            })),
            stage,
            queued: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            accepted: Arc::default(),
            gate: Arc::new(Semaphore::new(0)),
            task: Arc::default(),
            after_settle: Arc::default(),
            lose_settle_reply: Arc::default(),
        };
        let persistence = RouteConfigPersistence::for_test(&root, &routes).unwrap();
        let journal = TransactionJournal::new(persistence.clone());
        let runtime = RouteConfigRuntime::with_backend(
            store,
            persistence.clone(),
            Arc::new(backend.clone()),
            true,
        );
        Self {
            runtime,
            backend,
            persistence,
            journal,
            routes,
            original_yaml,
        }
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        if let Some(task) = self.backend.task.lock().unwrap().take() {
            task.abort();
        }
    }
}
pub(super) fn document(key: &str) -> RouteConfigYaml {
    serde_json::from_value(
        serde_json::json!({"providers":[{"id":"fixture","base_url":"https://example.invalid/v1",
        "credentials":[{"id":"account","api_key":format!("synthetic-{key}")}]}]}),
    )
    .unwrap()
}
