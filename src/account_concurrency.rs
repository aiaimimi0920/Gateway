//! Optional, exact-account hard ceiling in addition to provider-wide AIMD.
//! Local admission is atomic; Server mode coordinates across processes via one
//! dedicated PostgreSQL session-advisory-lock connection (never holding pool
//! connections or open transactions from `state.pg_pool`).
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Weak},
    time::Duration,
};

use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgConnectOptions, ConnectOptions, PgConnection, PgPool};
use tokio::sync::{mpsc, oneshot, watch};

use crate::{error::GatewayError, routing::candidate::RouteCandidate, state::AppState};

const MAX_IDENTITIES: usize = 8192;
const PG_POLL_INTERVAL: Duration = Duration::from_millis(100);
const PG_OP_TIMEOUT: Duration = Duration::from_secs(5);
const PG_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const PG_CASE_CHUNK: usize = 32;

#[derive(Debug, Default)]
pub(crate) struct AccountConcurrency {
    gates: Mutex<HashMap<(String, String), Weak<LocalGate>>>,
    shared: Mutex<Option<SharedCoordinatorHandle>>,
}

#[derive(Debug)]
struct LocalGate {
    active: Mutex<usize>,
    changed: watch::Sender<()>,
}

impl LocalGate {
    fn new() -> Self {
        let (changed, _) = watch::channel(());
        Self {
            active: Mutex::new(0),
            changed,
        }
    }

    async fn acquire(self: &Arc<Self>, limit: usize) -> LocalPermit {
        let mut changed = self.changed.subscribe();
        loop {
            {
                let mut active = self.active.lock();
                if *active < limit {
                    *active += 1;
                    return LocalPermit(Arc::clone(self));
                }
            }
            // The subscription predates the check, so a release cannot be lost.
            changed
                .changed()
                .await
                .expect("account gate sender is retained");
        }
    }
}

struct LocalPermit(Arc<LocalGate>);

impl Drop for LocalPermit {
    fn drop(&mut self) {
        let mut active = self.0.active.lock();
        assert!(*active > 0, "account concurrency permit released twice");
        *active -= 1;
        drop(active);
        self.0.changed.send_replace(());
    }
}

#[derive(Debug, Clone)]
struct SharedCoordinatorHandle {
    tx: mpsc::UnboundedSender<SharedCommand>,
}

impl SharedCoordinatorHandle {
    async fn try_acquire(&self, keys: Vec<i64>) -> Result<Option<SharedPermit>, GatewayError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(SharedCommand::TryAcquire {
                keys,
                reply: reply_tx,
                release_tx: self.tx.clone(),
            })
            .map_err(|_| storage_unavailable())?;
        reply_rx.await.map_err(|_| storage_unavailable())?
    }
}

enum SharedCommand {
    TryAcquire {
        keys: Vec<i64>,
        reply: oneshot::Sender<Result<Option<SharedPermit>, GatewayError>>,
        release_tx: mpsc::UnboundedSender<SharedCommand>,
    },
    Release {
        epoch: u64,
        key: i64,
        // Kept alive until PG unlock finishes so local waiters do not race an
        // in-flight unlock, and dropped immediately if the channel is closed.
        local: Option<LocalPermit>,
    },
}

struct SharedPermit {
    epoch: u64,
    key: i64,
    tx: mpsc::UnboundedSender<SharedCommand>,
    armed: bool,
}

impl SharedPermit {
    fn release(mut self, local: Option<LocalPermit>) {
        self.armed = false;
        if let Err(error) = self.tx.send(SharedCommand::Release {
            epoch: self.epoch,
            key: self.key,
            local,
        }) {
            drop(error.0);
        }
    }
}

impl Drop for SharedPermit {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.tx.send(SharedCommand::Release {
                epoch: self.epoch,
                key: self.key,
                local: None,
            });
        }
    }
}

pub(crate) struct AccountPermit {
    shared: Option<SharedPermit>,
    local: Option<LocalPermit>,
}

impl Drop for AccountPermit {
    fn drop(&mut self) {
        let local = self.local.take();
        if let Some(shared) = self.shared.take() {
            shared.release(local);
        } else {
            drop(local);
        }
    }
}

struct PgCoordinatorState {
    options: Arc<PgConnectOptions>,
    conn: Option<PgConnection>,
    epoch: u64,
    held_keys: HashSet<i64>,
}

impl PgCoordinatorState {
    fn new(options: Arc<PgConnectOptions>) -> Self {
        Self {
            options,
            conn: None,
            epoch: 1,
            held_keys: HashSet::new(),
        }
    }

    fn reset_connection(&mut self) {
        // Dropping PgConnection closes the TCP socket, releasing all session
        // advisory locks on PostgreSQL. Bumping epoch prevents stale Drop
        // messages from unlocking keys acquired on a subsequent session.
        self.conn = None;
        self.epoch = self.epoch.wrapping_add(1);
        self.held_keys.clear();
    }

    async fn ensure_connected(&mut self) -> Result<&mut PgConnection, GatewayError> {
        if self.conn.is_none() {
            let conn = tokio::time::timeout(PG_OP_TIMEOUT, self.options.as_ref().connect())
                .await
                .map_err(|_| storage_unavailable())?
                .map_err(|_| storage_unavailable())?;
            self.conn = Some(conn);
        }
        Ok(self.conn.as_mut().expect("connection initialized"))
    }

    async fn try_acquire_one(&mut self, keys: &[i64]) -> Result<Option<i64>, GatewayError> {
        let candidates = filter_unheld_keys(keys, &self.held_keys);
        if candidates.is_empty() {
            return Ok(None);
        }
        for chunk in candidates.chunks(PG_CASE_CHUNK) {
            let sql = build_try_acquire_sql(chunk.len());
            let result = {
                let conn = self.ensure_connected().await?;
                let mut query = sqlx::query_scalar::<_, Option<i64>>(&sql);
                for &key in chunk {
                    query = query.bind(key);
                }
                tokio::time::timeout(PG_OP_TIMEOUT, query.fetch_one(conn)).await
            };
            match result {
                Ok(Ok(Some(acquired))) => {
                    self.held_keys.insert(acquired);
                    return Ok(Some(acquired));
                }
                Ok(Ok(None)) => continue,
                Ok(Err(_)) | Err(_) => {
                    self.reset_connection();
                    return Err(storage_unavailable());
                }
            }
        }
        Ok(None)
    }

    async fn unlock(&mut self, key: i64) {
        let Some(conn) = self.conn.as_mut() else {
            return;
        };
        let result = tokio::time::timeout(
            PG_OP_TIMEOUT,
            sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock($1)")
                .bind(key)
                .fetch_one(conn),
        )
        .await;
        if !matches!(result, Ok(Ok(_))) {
            self.reset_connection();
        }
    }
}

async fn run_pg_coordinator(
    options: Arc<PgConnectOptions>,
    mut rx: mpsc::UnboundedReceiver<SharedCommand>,
) {
    let mut state = PgCoordinatorState::new(options);
    loop {
        let cmd = if state.conn.is_some() && state.held_keys.is_empty() {
            match tokio::time::timeout(PG_IDLE_TIMEOUT, rx.recv()).await {
                Ok(Some(cmd)) => cmd,
                Ok(None) => break,
                Err(_) => {
                    state.reset_connection();
                    continue;
                }
            }
        } else {
            match rx.recv().await {
                Some(cmd) => cmd,
                None => break,
            }
        };

        match cmd {
            SharedCommand::TryAcquire {
                keys,
                reply,
                release_tx,
            } => {
                if reply.is_closed() {
                    continue;
                }
                match state.try_acquire_one(&keys).await {
                    Ok(Some(key)) => {
                        let permit = SharedPermit {
                            epoch: state.epoch,
                            key,
                            tx: release_tx,
                            armed: true,
                        };
                        if let Err(Ok(Some(permit))) = reply.send(Ok(Some(permit))) {
                            // A cancelled receiver must still release the session lock.
                            permit.release(None);
                        }
                    }
                    Ok(None) => {
                        let _ = reply.send(Ok(None));
                    }
                    Err(error) => {
                        let _ = reply.send(Err(error));
                    }
                }
            }
            SharedCommand::Release { epoch, key, local } => {
                if epoch == state.epoch && state.held_keys.remove(&key) {
                    state.unlock(key).await;
                }
                drop(local);
            }
        }
    }
}

fn storage_unavailable() -> GatewayError {
    GatewayError::service_unavailable("Account concurrency storage unavailable")
}

fn filter_unheld_keys(keys: &[i64], held: &HashSet<i64>) -> Vec<i64> {
    keys.iter()
        .copied()
        .filter(|key| !held.contains(key))
        .collect()
}

fn build_try_acquire_sql(count: usize) -> String {
    debug_assert!(count > 0);
    // PostgreSQL CASE evaluates WHEN branches strictly left-to-right and stops
    // at the first match, acquiring at most one advisory lock per round trip.
    let mut sql = String::from("SELECT CASE");
    for idx in 1..=count {
        sql.push_str(&format!(
            " WHEN pg_try_advisory_lock(${idx}) THEN ${idx}::bigint"
        ));
    }
    sql.push_str(" ELSE NULL::bigint END");
    sql
}

fn account_key(candidate: &RouteCandidate) -> &str {
    candidate
        .payload
        .credential_id
        .as_deref()
        .or(candidate.provider_credential_id.as_deref())
        .unwrap_or(&candidate.provider_account_id)
}

impl AccountConcurrency {
    fn gate(&self, provider: &str, account: &str) -> Result<Arc<LocalGate>, GatewayError> {
        let mut gates = self.gates.lock();
        let key = (provider.to_owned(), account.to_owned());
        if let Some(gate) = gates.get(&key).and_then(Weak::upgrade) {
            return Ok(gate);
        }
        if gates.len() >= MAX_IDENTITIES {
            gates.retain(|_, weak| weak.strong_count() > 0);
        }
        if gates.len() >= MAX_IDENTITIES {
            return Err(GatewayError::service_unavailable(
                "Account concurrency registry is full",
            ));
        }
        let gate = Arc::new(LocalGate::new());
        gates.insert(key, Arc::downgrade(&gate));
        Ok(gate)
    }

    fn shared_coordinator(&self, pool: &PgPool) -> SharedCoordinatorHandle {
        let mut slot = self.shared.lock();
        if let Some(existing) = slot.as_ref() {
            if !existing.tx.is_closed() {
                return existing.clone();
            }
        }
        let (tx, rx) = mpsc::unbounded_channel();
        let options = pool.connect_options();
        tokio::spawn(run_pg_coordinator(options, rx));
        let handle = SharedCoordinatorHandle { tx };
        *slot = Some(handle.clone());
        handle
    }

    pub(crate) async fn acquire(
        &self,
        state: &AppState,
        candidate: &RouteCandidate,
    ) -> Result<Option<AccountPermit>, GatewayError> {
        let Some(limit) = candidate.max_concurrent_requests else {
            return Ok(None);
        };
        let account = account_key(candidate);
        let gate = self.gate(&candidate.provider_account_id, account)?;
        let local = gate.acquire(usize::from(limit)).await;
        let shared = if let Some(pool) = &state.pg_pool {
            let coordinator = self.shared_coordinator(pool);
            let keys: Vec<i64> = (0..limit)
                .map(|slot| lock_key(&candidate.provider_account_id, account, slot))
                .collect();
            let mut changed = gate.changed.subscribe();
            loop {
                match coordinator.try_acquire(keys.clone()).await? {
                    Some(permit) => break Some(permit),
                    None => {
                        tokio::select! {
                            _ = tokio::time::sleep(PG_POLL_INTERVAL) => {}
                            _ = changed.changed() => {}
                        }
                    }
                }
            }
        } else {
            None
        };
        Ok(Some(AccountPermit {
            shared,
            local: Some(local),
        }))
    }
}

pub(crate) fn limit_from_payload(payload: &serde_json::Value) -> Result<Option<u16>, GatewayError> {
    let Some(value) = payload
        .get("max_concurrent_requests")
        .filter(|value| !value.is_null())
    else {
        return Ok(None);
    };
    let Some(limit) = value.as_u64().and_then(|value| u16::try_from(value).ok()) else {
        return Err(GatewayError::service_unavailable(
            "Invalid account concurrency limit",
        ));
    };
    if !(1..=1000).contains(&limit) {
        return Err(GatewayError::service_unavailable(
            "Invalid account concurrency limit",
        ));
    }
    Ok(Some(limit))
}

fn lock_key(provider: &str, account: &str, slot: u16) -> i64 {
    let mut digest = Sha256::new();
    digest.update(b"gateway-account-concurrency-v1\0");
    digest.update((provider.len() as u32).to_be_bytes());
    digest.update(provider.as_bytes());
    digest.update((account.len() as u32).to_be_bytes());
    digest.update(account.as_bytes());
    digest.update(slot.to_be_bytes());
    i64::from_be_bytes(digest.finalize()[..8].try_into().expect("SHA-256 length"))
}

#[cfg(test)]
#[path = "account_concurrency/tests.rs"]
mod tests;
