// ---------------------------------------------------------------------------
// AimdController — Additive-Increase / Multiplicative-Decrease concurrency limiter
//
// Ported from NeuroLoom nl_llm concurrency controller.
// Uses a Tokio Semaphore as the permit source plus a "withheld permits" pool
// that absorbs capacity when the limit is decreased, preventing new work from
// filling slots that should no longer be available.
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AimdConfig {
    /// Starting concurrency limit. Default: 10.
    pub initial_limit: usize,
    /// Floor — limit will never drop below this. Default: 1.
    pub min_limit: usize,
    /// Ceiling — limit will never exceed this. Default: 100.
    pub max_limit: usize,
    /// How many consecutive successes trigger a +1 increase. Default: 5.
    pub increase_threshold: usize,
    /// Multiplicative factor applied on a general failure. Default: 0.7.
    pub decrease_factor: f64,
    /// Multiplicative factor applied on a rate-limit failure. Default: 0.5.
    pub severe_decrease_factor: f64,
}

impl Default for AimdConfig {
    fn default() -> Self {
        Self {
            initial_limit: 10,
            min_limit: 1,
            max_limit: 100,
            increase_threshold: 5,
            decrease_factor: 0.7,
            severe_decrease_factor: 0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// FailureKind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// Generic error (server error, network failure, etc.).
    General,
    /// HTTP 429 / provider rate limit.
    RateLimited,
}

// ---------------------------------------------------------------------------
// Internals — withheld permit pool
// ---------------------------------------------------------------------------

struct WithheldPool {
    /// Owned semaphore permits that have been removed from the active pool.
    held: Vec<OwnedSemaphorePermit>,
    /// Permits that need to be acquired from the semaphore but have not yet
    /// been obtained (because the semaphore had no available permits at the
    /// time of the decrease).
    pending: usize,
}

// ---------------------------------------------------------------------------
// ConcurrencySnapshot
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ConcurrencySnapshot {
    pub current_limit: usize,
    pub active_count: usize,
    pub available: usize,
    pub consecutive_successes: usize,
}

// ---------------------------------------------------------------------------
// AimdController
// ---------------------------------------------------------------------------

pub struct AimdController {
    config: AimdConfig,
    semaphore: Arc<Semaphore>,
    current_limit: AtomicUsize,
    active_count: AtomicUsize,
    consecutive_successes: AtomicUsize,
    withheld: Mutex<WithheldPool>,
}

impl AimdController {
    pub fn new(config: AimdConfig) -> Self {
        let initial = config.initial_limit;
        Self {
            config,
            semaphore: Arc::new(Semaphore::new(initial)),
            current_limit: AtomicUsize::new(initial),
            active_count: AtomicUsize::new(0),
            consecutive_successes: AtomicUsize::new(0),
            withheld: Mutex::new(WithheldPool {
                held: Vec::new(),
                pending: 0,
            }),
        }
    }

    // ── permit acquisition ───────────────────────────────────────────────

    /// Asynchronously wait for a concurrency slot and return an RAII permit.
    pub async fn acquire(self: &Arc<Self>) -> ConcurrencyPermit {
        let owned = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore should never be closed");
        self.active_count.fetch_add(1, Ordering::Relaxed);
        ConcurrencyPermit::new(Arc::clone(self), owned)
    }

    /// Non-blocking attempt to acquire a concurrency slot. Returns `None` if
    /// all slots are occupied.
    pub fn try_acquire(self: &Arc<Self>) -> Option<ConcurrencyPermit> {
        let owned = self.semaphore.clone().try_acquire_owned().ok()?;
        self.active_count.fetch_add(1, Ordering::Relaxed);
        Some(ConcurrencyPermit::new(Arc::clone(self), owned))
    }

    // ── feedback reporting ───────────────────────────────────────────────

    /// Call after a successful upstream response. Triggers an additive increase
    /// once `increase_threshold` consecutive successes have been observed.
    pub fn on_success(&self) {
        let prev = self.consecutive_successes.fetch_add(1, Ordering::Relaxed);
        let next = prev + 1;

        if next >= self.config.increase_threshold {
            // Reset the counter and increase the limit.
            self.consecutive_successes.store(0, Ordering::Relaxed);
            self.increase_limit();
        }
    }

    /// Call after an upstream failure. Applies the appropriate multiplicative
    /// decrease and resets the consecutive-success counter.
    pub fn on_failure(&self, kind: FailureKind) {
        self.consecutive_successes.store(0, Ordering::Relaxed);

        let factor = match kind {
            FailureKind::RateLimited => self.config.severe_decrease_factor,
            FailureKind::General => self.config.decrease_factor,
        };
        self.decrease_limit(factor);
    }

    // ── snapshot ─────────────────────────────────────────────────────────

    pub fn snapshot(&self) -> ConcurrencySnapshot {
        let current_limit = self.current_limit.load(Ordering::Relaxed);
        let active_count = self.active_count.load(Ordering::Relaxed);
        let available = current_limit.saturating_sub(active_count);
        ConcurrencySnapshot {
            current_limit,
            active_count,
            available,
            consecutive_successes: self.consecutive_successes.load(Ordering::Relaxed),
        }
    }

    // ── internal helpers ─────────────────────────────────────────────────

    fn increase_limit(&self) {
        let current = self.current_limit.load(Ordering::Relaxed);
        if current >= self.config.max_limit {
            return;
        }
        let new_limit = (current + 1).min(self.config.max_limit);
        self.current_limit.store(new_limit, Ordering::Relaxed);
        // Release withheld capacity before adding fresh permits.
        self.release_withheld(new_limit - current);
    }

    fn decrease_limit(&self, factor: f64) {
        let current = self.current_limit.load(Ordering::Relaxed);
        let new_limit = ((current as f64 * factor).floor() as usize)
            .max(self.config.min_limit)
            .min(current);

        if new_limit >= current {
            return;
        }

        self.current_limit.store(new_limit, Ordering::Relaxed);
        let reduction = current - new_limit;

        {
            let mut pool = self.withheld.lock();
            pool.pending += reduction;
        }

        // Eagerly try to absorb idle semaphore permits now.
        self.reconcile_withheld();
    }

    /// Move idle semaphore permits into the withheld pool until `pending`
    /// reaches zero or the semaphore is exhausted.
    fn reconcile_withheld(&self) {
        let mut pool = self.withheld.lock();
        while pool.pending > 0 {
            match self.semaphore.clone().try_acquire_owned() {
                Ok(permit) => {
                    pool.held.push(permit);
                    pool.pending -= 1;
                }
                Err(_) => break,
            }
        }
    }

    /// Release `amount` withheld permits back into the active pool, consuming
    /// pending bookings first before actually releasing held permits.
    fn release_withheld(&self, mut amount: usize) {
        if amount == 0 {
            return;
        }

        let mut pool = self.withheld.lock();

        // Cancel pending bookings first — they haven't consumed an actual
        // semaphore permit yet so no release action is needed.
        let cancel = amount.min(pool.pending);
        pool.pending -= cancel;
        amount -= cancel;

        // Release held permits back to the semaphore.
        while amount > 0 && !pool.held.is_empty() {
            pool.held.pop(); // drop → semaphore.add_permits(1) automatically
            amount -= 1;
        }

        drop(pool);

        // If we consumed pending bookings without matching held permits,
        // those slots simply never existed in the withheld pool — they will
        // become naturally available because we are not acquiring them.
        // Any extra `amount` remaining here means neither pending nor held
        // were enough; add fresh permits directly.
        if amount > 0 {
            self.semaphore.add_permits(amount);
        }
    }

    /// Called by `ConcurrencyPermit` on drop to decrement the active count
    /// and reconcile any pending withheld permits.
    pub(crate) fn on_permit_dropped(&self) {
        self.active_count.fetch_sub(1, Ordering::Relaxed);
        // A slot just became free — try to absorb it if still pending.
        self.reconcile_withheld();
    }
}

// ---------------------------------------------------------------------------
// ConcurrencyPermit — RAII guard
// ---------------------------------------------------------------------------

/// Holds a semaphore permit for the duration of a single upstream request.
/// Dropping this value returns the permit to the semaphore (or withheld pool)
/// and decrements the active-request counter.
pub struct ConcurrencyPermit {
    controller: Arc<AimdController>,
    // Option so we can take the permit on drop to control release order.
    _permit: Option<OwnedSemaphorePermit>,
}

impl ConcurrencyPermit {
    fn new(controller: Arc<AimdController>, permit: OwnedSemaphorePermit) -> Self {
        Self {
            controller,
            _permit: Some(permit),
        }
    }
}

impl Drop for ConcurrencyPermit {
    fn drop(&mut self) {
        // Release the semaphore permit first so the slot becomes available.
        drop(self._permit.take());
        // Then notify the controller (may absorb the permit into withheld pool
        // via reconcile_withheld, but the drop above already happened, so the
        // semaphore count is incremented before reconcile runs).
        self.controller.on_permit_dropped();
    }
}

// Safety: AimdController uses only thread-safe primitives.
unsafe impl Send for ConcurrencyPermit {}
unsafe impl Sync for ConcurrencyPermit {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_controller(
        initial: usize,
        min: usize,
        max: usize,
        threshold: usize,
    ) -> Arc<AimdController> {
        Arc::new(AimdController::new(AimdConfig {
            initial_limit: initial,
            min_limit: min,
            max_limit: max,
            increase_threshold: threshold,
            decrease_factor: 0.7,
            severe_decrease_factor: 0.5,
        }))
    }

    #[test]
    fn initial_snapshot_reflects_config() {
        let ctrl = make_controller(10, 1, 100, 5);
        let snap = ctrl.snapshot();
        assert_eq!(snap.current_limit, 10);
        assert_eq!(snap.active_count, 0);
        assert_eq!(snap.available, 10);
        assert_eq!(snap.consecutive_successes, 0);
    }

    #[test]
    fn try_acquire_decrements_available() {
        let ctrl = make_controller(4, 1, 10, 5);
        let _p1 = ctrl.try_acquire().expect("permit 1");
        let _p2 = ctrl.try_acquire().expect("permit 2");
        let snap = ctrl.snapshot();
        assert_eq!(snap.active_count, 2);
        assert_eq!(snap.available, 2);
    }

    #[test]
    fn drop_permit_restores_active_count() {
        let ctrl = make_controller(4, 1, 10, 5);
        {
            let _p = ctrl.try_acquire().expect("permit");
            assert_eq!(ctrl.snapshot().active_count, 1);
        }
        assert_eq!(ctrl.snapshot().active_count, 0);
    }

    #[test]
    fn rate_limit_failure_halves_limit() {
        let ctrl = make_controller(10, 1, 100, 5);
        ctrl.on_failure(FailureKind::RateLimited);
        // floor(10 * 0.5) = 5
        assert_eq!(ctrl.snapshot().current_limit, 5);
    }

    #[test]
    fn general_failure_applies_decrease_factor() {
        let ctrl = make_controller(10, 1, 100, 5);
        ctrl.on_failure(FailureKind::General);
        // floor(10 * 0.7) = 7
        assert_eq!(ctrl.snapshot().current_limit, 7);
    }

    #[test]
    fn limit_never_drops_below_min() {
        let ctrl = make_controller(1, 1, 100, 5);
        ctrl.on_failure(FailureKind::RateLimited);
        assert_eq!(ctrl.snapshot().current_limit, 1);
    }

    #[test]
    fn consecutive_successes_increase_limit() {
        let ctrl = make_controller(5, 1, 100, 3);
        // 3 successes → +1
        ctrl.on_success();
        ctrl.on_success();
        ctrl.on_success();
        assert_eq!(ctrl.snapshot().current_limit, 6);
    }

    #[test]
    fn failure_resets_consecutive_success_counter() {
        let ctrl = make_controller(5, 1, 100, 5);
        ctrl.on_success();
        ctrl.on_success();
        ctrl.on_failure(FailureKind::General);
        assert_eq!(ctrl.snapshot().consecutive_successes, 0);
    }

    #[test]
    fn limit_never_exceeds_max() {
        let ctrl = make_controller(10, 1, 10, 1);
        // Every success triggers +1 but limit should stay at 10.
        ctrl.on_success();
        assert_eq!(ctrl.snapshot().current_limit, 10);
    }

    #[test]
    fn withheld_permits_block_beyond_new_limit() {
        let ctrl = make_controller(4, 1, 10, 5);
        ctrl.on_failure(FailureKind::RateLimited); // limit → 2
        let snap = ctrl.snapshot();
        assert_eq!(snap.current_limit, 2);

        // Only 2 permits should be gettable.
        let p1 = ctrl.try_acquire().expect("permit 1");
        let p2 = ctrl.try_acquire().expect("permit 2");
        assert!(
            ctrl.try_acquire().is_none(),
            "third permit should be blocked"
        );
        drop(p1);
        drop(p2);
    }

    #[test]
    fn release_withheld_on_increase_after_failure() {
        let ctrl = make_controller(4, 1, 10, 1);
        ctrl.on_failure(FailureKind::RateLimited); // limit 4 → 2
        ctrl.on_success(); // limit 2 → 3

        // Should be able to acquire 3 permits.
        let p1 = ctrl.try_acquire().expect("permit 1");
        let p2 = ctrl.try_acquire().expect("permit 2");
        let p3 = ctrl.try_acquire().expect("permit 3");
        assert!(ctrl.try_acquire().is_none(), "fourth should be blocked");
        drop(p1);
        drop(p2);
        drop(p3);
    }

    #[tokio::test]
    async fn async_acquire_works() {
        let ctrl = make_controller(2, 1, 10, 5);
        let p1 = ctrl.acquire().await;
        let p2 = ctrl.acquire().await;
        assert_eq!(ctrl.snapshot().active_count, 2);
        drop(p1);
        drop(p2);
        assert_eq!(ctrl.snapshot().active_count, 0);
    }
}
