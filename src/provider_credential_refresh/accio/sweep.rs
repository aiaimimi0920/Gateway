//! One serial DB worker: bounded keyset scans, permanent admission, result-first persistence.
use super::{failed, is_due, received::Received, send};
use crate::{
    console::refresh_grants::{GrantPhase, GrantReservation},
    db::provider_credentials::refresh as store,
    error::GatewayError,
    provider_credential_refresh::ProviderCredentialRefreshSweepSummary as Summary,
    state::GatewayShutdownHandle,
};
use futures::{future::BoxFuture, FutureExt};
use sqlx::PgPool;
use std::{future::Future, time::Duration};
use time::OffsetDateTime;

const STORAGE_BUDGET: Duration = Duration::from_secs(10);

pub(crate) struct Worker {
    pool: PgPool,
    http: rquest::Client,
    shutdown: GatewayShutdownHandle,
    limit: i64,
    margin: Duration,
    cursor: Option<String>,
    received: Option<Received>,
}

#[derive(Clone, Copy)]
enum Disposition {
    Fresh,
    Published,
    Held,
    Stale,
}

impl Worker {
    pub(crate) fn new(
        pool: PgPool,
        http: rquest::Client,
        shutdown: GatewayShutdownHandle,
        limit: i64,
        margin: Duration,
    ) -> Self {
        Self {
            pool,
            http,
            shutdown,
            limit: limit.max(1),
            margin,
            cursor: None,
            received: None,
        }
    }

    pub(crate) async fn run(mut self, interval: Duration) {
        while !self.shutdown.is_requested() {
            if let Err(error) = self.tick().await {
                report(&error);
            }
            tokio::select! {
                _ = self.shutdown.wait() => break,
                _ = tokio::time::sleep(interval) => {}
            }
        }
        // The task is joined; shutdown makes one bounded persistence-only attempt.
        // A process kill can lose unsaved response bytes, but never restores send permission.
        if let Some(received) = &self.received {
            if let Err(error) = bounded(store::store_result(
                &self.pool,
                &received.sending,
                &received.result,
            ))
            .await
            {
                report(&error);
            }
        }
    }

    pub(crate) async fn tick(&mut self) -> Result<Summary, GatewayError> {
        let mut summary = Summary::default();
        if self.received.is_some() {
            let disposition = Box::pin(self.persist_received()).await?;
            count(&mut summary, disposition);
        }
        for _ in 0..self.limit {
            if self.shutdown.is_requested() {
                break;
            }
            let Some(id) = bounded(store::next_active(&self.pool, self.cursor.as_deref())).await?
            else {
                self.cursor = None;
                break;
            };
            self.cursor = Some(id.clone());
            summary.scanned_count += 1;
            match Box::pin(self.process(&id, &mut summary)).await {
                Ok(disposition) => count(&mut summary, disposition),
                Err(error) => {
                    summary.failed_count += 1;
                    report(&error);
                }
            }
            if self.received.is_some() {
                break;
            }
        }
        Ok(summary)
    }

    async fn process(
        &mut self,
        id: &str,
        summary: &mut Summary,
    ) -> Result<Disposition, GatewayError> {
        let Some(snapshot) = bounded(store::read_snapshot(&self.pool, id)).await? else {
            return Ok(Disposition::Stale);
        };
        let due = is_due(&snapshot, OffsetDateTime::now_utc(), self.margin);
        summary.due_count += usize::from(due);
        // Recover an acknowledged Result before due filtering; a changed interval/margin
        // cannot strand a result or turn it into permission to send another request.
        if let Some(record) = bounded(store::load_current(&self.pool, &snapshot)).await? {
            if !record.matches_request(&snapshot.request) || record.phase() != GrantPhase::Result {
                return Ok(Disposition::Held);
            }
            let next = bounded(store::publish_result(&self.pool, &record)).await?;
            return Ok(if next.is_some() {
                Disposition::Published
            } else {
                Disposition::Stale
            });
        }
        if !due {
            return Ok(Disposition::Fresh);
        }
        // Drain may arrive during the read-only lookup. Do not burn a new Sending
        // after stopping; an existing Result above may still finish disk-only work.
        if self.shutdown.is_requested() {
            return Ok(Disposition::Stale);
        }
        let prepared = send::prepare(&snapshot)?;
        let sending = match bounded(store::reserve(&self.pool, &snapshot)).await? {
            GrantReservation::Reserved(record) => record,
            GrantReservation::Existing(record) if record.phase() == GrantPhase::Result => {
                let next = bounded(store::publish_result(&self.pool, &record)).await?;
                return Ok(if next.is_some() {
                    Disposition::Published
                } else {
                    Disposition::Stale
                });
            }
            GrantReservation::Stale => return Ok(Disposition::Stale),
            _ => return Ok(Disposition::Held),
        };
        if self.shutdown.is_requested() {
            return Ok(Disposition::Stale);
        }
        let Some(authority) =
            bounded(store::authorize_send(&self.pool, &snapshot, &sending)).await?
        else {
            return Ok(Disposition::Stale);
        };
        if self.shutdown.is_requested() {
            return Ok(Disposition::Stale);
        }
        // No generic form, retry policy, TTL lock or detached sender enters this path.
        let tokens = Box::pin(prepared.execute(&self.http))
            .await
            .map_err(|_| failed("accio_db_steward_unknown_send"))?;
        self.received = Some(Received {
            result: send::result(&snapshot, tokens)?,
            sending,
        });
        // Material is retained before releasing permission locks or awaiting any DB write.
        // SQLx drains the read-only transaction rollback before reusing its connection.
        drop(authority);
        Box::pin(self.persist_received()).await
    }

    async fn persist_received(&mut self) -> Result<Disposition, GatewayError> {
        let received = self
            .received
            .as_ref()
            .expect("one confirmed DB Accio result");
        let accepted = bounded(store::store_result(
            &self.pool,
            &received.sending,
            &received.result,
        ))
        .await?;
        // Only acknowledged durable Result clears the memory slot. Publication failure
        // is recovered from the original journal, including after this worker is dropped.
        self.received = None;
        let next = bounded(store::publish_result(&self.pool, &accepted)).await?;
        Ok(if next.is_some() {
            Disposition::Published
        } else {
            Disposition::Stale
        })
    }

    #[cfg(test)]
    pub(crate) fn has_received(&self) -> bool {
        self.received.is_some()
    }
}

fn bounded<'a, T: Send + 'a>(
    operation: impl Future<Output = Result<T, GatewayError>> + Send + 'a,
) -> BoxFuture<'a, Result<T, GatewayError>> {
    // SQLx/object/HTTP futures are large. Keep each stage on the heap instead of
    // nesting their concrete states in the worker's default Windows thread stack.
    // This pins the original operation; it spawns no task and changes no retry/Drop semantics.
    let operation = Box::pin(operation);
    async move {
        tokio::time::timeout(STORAGE_BUDGET, operation)
            .await
            .map_err(|_| failed("accio_db_steward_storage_unknown"))?
    }
    .boxed()
}
fn report(error: &GatewayError) {
    tracing::warn!(
        code = error
            .code
            .as_deref()
            .unwrap_or("accio_db_steward_storage_failed"),
        "Accio DB credential refresh did not complete"
    );
}
fn count(summary: &mut Summary, disposition: Disposition) {
    match disposition {
        Disposition::Published => summary.refreshed_count += 1,
        Disposition::Held => summary.locked_count += 1,
        Disposition::Fresh | Disposition::Stale => summary.skipped_count += 1,
    }
}
