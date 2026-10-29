//! A bounded undo journal for one process-wide egress incident. Lock entries before this journal.
use super::*;

#[derive(Debug, Default)]
pub(super) struct NetworkWindow {
    pub enabled: bool,
    pub started: bool,
    pub checking: bool,
    pub unstable: bool,
    pub epoch: u64,
    pub checked_at: Option<u64>,
    pub probe_in_flight: bool,
    pub baseline: HashMap<CircuitKey, Entry>,
    pub recorded_failures: u64,
    pub last_rollback: usize,
    pub recovered_at: Option<Instant>,
    pub recovery_generation: u64,
}

impl NetworkWindow {
    pub fn active(&self) -> bool {
        self.checking || self.unstable
    }
    pub fn observe(
        &mut self,
        key: &CircuitKey,
        before: Entry,
        after: &Entry,
        reversible: bool,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        let tripped =
            before.generation != after.generation && matches!(after.phase, Phase::Open(_));
        let start = !self.active() && tripped;
        if start {
            self.checking = true;
            self.epoch = self.epoch.wrapping_add(1);
            self.baseline.clear();
            self.recorded_failures = 0;
        }
        if self.active() {
            if reversible {
                self.baseline.entry(key.clone()).or_insert(before);
            } else {
                // A confirmed upstream refusal must not be erased by unrelated egress recovery.
                self.baseline.remove(key);
            }
            self.recorded_failures = self.recorded_failures.saturating_add(1);
        }
        start
    }
}

impl ModelCircuits {
    #[cfg(test)]
    pub(crate) fn enable_network_test_guard(&self) {
        self.network.lock().enabled = true;
    }
    pub(crate) fn network_blocked(&self) -> bool {
        self.network.lock().active()
    }
    pub(crate) fn network_feedback_allowed(&self, started: Instant) -> bool {
        self.network
            .lock()
            .recovered_at
            .is_none_or(|at| started > at)
    }
    pub(crate) fn network_epoch(&self) -> Option<u64> {
        let network = self.network.lock();
        network.active().then_some(network.epoch)
    }
    pub(crate) fn network_error(&self) -> Option<crate::error::GatewayError> {
        self.network_blocked().then(|| {
            let mut error = crate::error::GatewayError::service_unavailable(
                "Gateway egress network is temporarily unstable",
            )
            .with_code("gateway_network_unstable");
            error.retryable = false;
            error.fallback_hint = crate::error::FallbackHint::Abort {
                reason: "gateway_network_unstable".into(),
            };
            error
        })
    }
    pub(crate) fn network_report(&self) -> serde_json::Value {
        let network = self.network.lock();
        serde_json::json!({"status":if network.unstable {"temporary_network_fluctuation"} else if network.checking {"checking"} else {"normal"},
            "label":if network.unstable {"临时网络波动"} else if network.checking {"正在检查出口网络"} else {"正常"},
            "epoch":network.epoch,"affected_circuits":network.baseline.len(),"recorded_failures":network.recorded_failures,
            "last_rollback_circuits":network.last_rollback,"checked_ago_ms":network.checked_at.map(|at|self.now().saturating_sub(at))})
    }
    pub(super) fn network_probe_due(&self) -> Option<u64> {
        let mut network = self.network.lock();
        if !network.active()
            || network.probe_in_flight
            || (network.unstable
                && network
                    .checked_at
                    .is_some_and(|at| self.now().saturating_sub(at) < 15_000))
        {
            return None;
        }
        network.probe_in_flight = true;
        Some(network.epoch)
    }
    pub(super) fn complete_network_probe(&self, epoch: u64, reachable: bool) {
        let mut entries = self.entries.lock();
        let mut network = self.network.lock();
        if epoch != network.epoch {
            return;
        }
        network.probe_in_flight = false;
        network.checked_at = Some(self.now());
        if !reachable {
            network.unstable = true;
            network.checking = false;
            return;
        }
        if network.unstable {
            network.recovered_at = Some(Instant::now());
            network.recovery_generation = network.recovery_generation.wrapping_add(1);
            network.last_rollback = network.baseline.len();
            for (key, mut before) in network.baseline.drain() {
                if let Some(entry) = entries.get_mut(&key) {
                    before.generation = entry.generation.wrapping_add(1);
                    before.in_flight = entry.in_flight;
                    before.touched = self.now();
                    if let Phase::Trial { lease, .. } = &mut before.phase {
                        *lease = None;
                    }
                    *entry = before;
                }
            }
        }
        network.baseline.clear();
        network.unstable = false;
        network.checking = false;
    }
    pub(crate) fn reset_provider(&self, provider: &str) {
        let mut entries = self.entries.lock();
        for (key, entry) in entries
            .iter_mut()
            .filter(|(key, _)| key.provider == provider)
        {
            let _ = key;
            let mut clean = Entry::new(self.now());
            clean.generation = entry.generation.wrapping_add(1);
            clean.in_flight = entry.in_flight;
            *entry = clean;
        }
        // Recovery must never resurrect a restriction explicitly cleared by the operator.
        self.network
            .lock()
            .baseline
            .retain(|key, _| key.provider != provider);
    }
}
