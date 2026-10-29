//! Bounded, in-memory credential/model backoff; never part of candidate filtering or hard circuits.
use super::CircuitKey;
use crate::error::{FallbackHint, GatewayError};
use std::collections::HashMap;
use uuid::Uuid;

const LIMIT: usize = 8192;

#[derive(Debug)]
pub(super) struct CapacityTicket {
    key: CircuitKey,
    generation: Uuid,
}

#[derive(Debug)]
struct Entry {
    generation: Uuid,
    in_flight: usize,
    level: usize,
    started: u64,
    delay: u64,
    error: Option<GatewayError>,
}
impl Entry {
    fn remaining(&self, now: u64) -> u64 {
        self.delay.saturating_sub(now.saturating_sub(self.started))
    }
}

#[derive(Debug, Default)]
pub(super) struct CapacityBackoff {
    entries: HashMap<CircuitKey, Entry>,
}
impl CapacityBackoff {
    pub(super) fn acquire(
        &mut self,
        key: &CircuitKey,
        now: u64,
    ) -> Result<CapacityTicket, GatewayError> {
        if key.provider.len() > 1024 || key.group.len() > 1024 || key.model.len() > 1024 {
            return Err(full());
        }
        if !self.entries.contains_key(key) && self.entries.len() >= LIMIT {
            // Never evict an active restriction or an in-flight generation.
            self.entries
                .retain(|_, e| e.in_flight > 0 || e.remaining(now) > 0);
            if self.entries.len() >= LIMIT {
                return Err(full());
            }
        }
        let entry = self.entries.entry(key.clone()).or_insert_with(|| Entry {
            generation: Uuid::new_v4(),
            in_flight: 0,
            level: 0,
            started: now,
            delay: 0,
            error: None,
        });
        if let Some(error) = entry.error.as_ref().filter(|_| entry.remaining(now) > 0) {
            let mut error = with_delay(error.clone(), entry.remaining(now));
            // Local refusal must not become another upstream observation or retry loop.
            error.retryable = false;
            return Err(error);
        }
        entry.in_flight += 1;
        Ok(CapacityTicket {
            key: key.clone(),
            generation: entry.generation,
        })
    }

    pub(super) fn failure(
        &mut self,
        ticket: CapacityTicket,
        error: &GatewayError,
        now: u64,
    ) -> GatewayError {
        let entry = self
            .entries
            .get_mut(&ticket.key)
            .expect("live capacity ticket");
        entry.in_flight -= 1;
        entry.level = (entry.level + 1).min(4);
        let delay = [2_000, 4_000, 8_000, 15_000][entry.level - 1]
            .max(error.upstream_retry_after_ms.unwrap_or(0))
            .max(entry.remaining(now));
        entry.started = now;
        entry.delay = delay;
        entry.generation = Uuid::new_v4();
        let error = with_delay(error.clone(), delay);
        entry.error = Some(error.clone());
        error
    }

    pub(super) fn finish(&mut self, ticket: CapacityTicket, success: bool) {
        let Some(entry) = self.entries.get_mut(&ticket.key) else {
            return;
        };
        entry.in_flight -= 1;
        // A late success predating a newer refusal cannot clear that refusal.
        if success && entry.generation == ticket.generation {
            entry.error = None;
            entry.level = 0;
            entry.delay = 0;
        }
        if entry.in_flight == 0 && entry.error.is_none() {
            self.entries.remove(&ticket.key);
        }
    }
}

fn with_delay(mut error: GatewayError, delay_ms: u64) -> GatewayError {
    error.fallback_hint = FallbackHint::Retry {
        delay_ms,
        reason: "Upstream capacity is temporarily unavailable.".into(),
    };
    error
}

fn full() -> GatewayError {
    let mut error = GatewayError::service_unavailable("Capacity backoff admission is full")
        .with_code("capacity_backoff_registry_full");
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "capacity_backoff_registry_full".into(),
    };
    error
}

#[cfg(test)]
#[path = "capacity_tests.rs"]
mod tests;
