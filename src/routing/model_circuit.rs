//! Model-level hard admission. Closed circuits are not routing scores; open ones are absent.
mod attempt;
#[cfg(test)]
mod balance_tests;
mod failure;
#[cfg(test)]
mod tests;
pub(crate) use attempt::CircuitAttempt;
#[cfg(test)]
use failure::failure_delay;
pub(crate) use failure::failure_outcome;
use parking_lot::Mutex;
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    time::Instant,
};
use uuid::Uuid;

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize)]
pub(crate) struct CircuitKey {
    pub provider: String,
    pub group: String,
    pub model: String,
}
#[derive(Debug)]
enum Phase {
    Closed,
    Open(u64),
    Trial {
        next: u64,
        successes: u8,
        lease: Option<Uuid>,
    },
}
#[derive(Debug)]
struct Entry {
    phase: Phase,
    generation: u64,
    level: u32,
    failures: VecDeque<u64>,
    stable_since: Option<u64>,
    touched: u64,
    in_flight: usize,
}
impl Entry {
    fn new(now: u64) -> Self {
        Self {
            phase: Phase::Closed,
            generation: 0,
            level: 0,
            failures: VecDeque::new(),
            stable_since: None,
            touched: now,
            in_flight: 0,
        }
    }
    fn available(&self, now: u64) -> bool {
        match self.phase {
            Phase::Closed => true,
            Phase::Open(until) => now >= until,
            Phase::Trial { next, lease, .. } => lease.is_none() && now >= next,
        }
    }
    fn trip(&mut self, now: u64, minimum_ms: u64, minimum_level: u32) {
        self.level = self.level.saturating_add(1).max(minimum_level).min(4);
        let delay = (120_000u64 * 4u64.pow(self.level - 1))
            .min(3_600_000)
            .max(minimum_ms);
        self.phase = Phase::Open(now.saturating_add(delay));
        self.generation = self.generation.wrapping_add(1);
        self.failures.clear();
        self.stable_since = None;
    }
}
#[derive(Clone, Debug)]
struct Ticket {
    key: CircuitKey,
    generation: u64,
    lease: Option<Uuid>,
}
#[derive(Clone, Copy)]
pub(crate) enum Outcome {
    Success,
    Failure(u64),
    ChannelUnavailable(u64),
    BalanceUnavailable(u64),
    Neutral,
}

#[derive(Debug)]
pub(crate) struct ModelCircuits {
    origin: Instant,
    entries: Mutex<HashMap<CircuitKey, Entry>>,
}
impl Default for ModelCircuits {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
            entries: Mutex::new(HashMap::new()),
        }
    }
}
impl ModelCircuits {
    fn now(&self) -> u64 {
        self.origin.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
    pub(crate) fn available(&self, key: &CircuitKey) -> bool {
        self.available_at(key, self.now())
    }
    fn available_at(&self, key: &CircuitKey, now: u64) -> bool {
        if key.provider.len() > 1024 || key.group.len() > 1024 || key.model.len() > 1024 {
            return false;
        }
        self.entries
            .lock()
            .get(key)
            .is_none_or(|entry| entry.available(now))
    }
    fn acquire_at(&self, keys: &[CircuitKey], now: u64) -> Result<Vec<Ticket>, ()> {
        if keys.iter().any(|key| {
            key.provider.len() > 1024 || key.group.len() > 1024 || key.model.len() > 1024
        }) {
            return Err(());
        }
        let mut entries = self.entries.lock();
        if keys
            .iter()
            .any(|key| entries.get(key).is_some_and(|e| !e.available(now)))
        {
            return Err(());
        }
        // Never evict active restrictions to admit a new model, which would bypass protection.
        if entries.len()
            + keys
                .iter()
                .filter(|key| !entries.contains_key(*key))
                .count()
            > 8192
        {
            entries.retain(|_, e| {
                if e.in_flight > 0 {
                    return true;
                }
                let age = now.saturating_sub(e.touched);
                match e.phase {
                    Phase::Closed => {
                        !(e.level == 0 && (e.failures.is_empty() || age > 60_000)
                            || age >= 86_400_000)
                    }
                    Phase::Open(until) => now < until || age < 86_400_000,
                    Phase::Trial { lease, .. } => lease.is_some() || age < 86_400_000,
                }
            });
            if entries.len()
                + keys
                    .iter()
                    .filter(|key| !entries.contains_key(*key))
                    .count()
                > 8192
            {
                return Err(());
            }
        }
        Ok(keys
            .iter()
            .map(|key| {
                let entry = entries
                    .entry(key.clone())
                    .or_insert_with(|| Entry::new(now));
                entry.touched = now;
                entry.in_flight += 1;
                if matches!(entry.phase, Phase::Open(_)) {
                    entry.phase = Phase::Trial {
                        next: now,
                        successes: 0,
                        lease: None,
                    };
                }
                let lease = match &mut entry.phase {
                    Phase::Trial { lease, .. } => {
                        *lease = Some(Uuid::new_v4());
                        *lease
                    }
                    _ => None,
                };
                Ticket {
                    key: key.clone(),
                    generation: entry.generation,
                    lease,
                }
            })
            .collect())
    }
    fn finish_at(&self, tickets: Vec<Ticket>, outcome: Outcome, now: u64) {
        let mut entries = self.entries.lock();
        for ticket in tickets {
            let Some(entry) = entries.get_mut(&ticket.key) else {
                continue;
            };
            entry.in_flight = entry.in_flight.saturating_sub(1);
            // Completions from the pre-trip generation cannot reopen or clear a newer circuit.
            if entry.generation != ticket.generation {
                continue;
            }
            if let Phase::Trial { lease, .. } = &entry.phase {
                if *lease != ticket.lease {
                    continue;
                }
            }
            entry.touched = now;
            // Only balance outcomes poison the account-wide ticket. Other failures retain
            // their existing model/group scope; successes can recover both trial types.
            let outcome = match (ticket.key.group.starts_with("balance:"), outcome) {
                (true, Outcome::BalanceUnavailable(ms)) => Outcome::ChannelUnavailable(ms),
                (false, Outcome::BalanceUnavailable(_)) => Outcome::Neutral,
                (true, Outcome::Failure(_) | Outcome::ChannelUnavailable(_)) => Outcome::Neutral,
                (_, other) => other,
            };
            match outcome {
                Outcome::ChannelUnavailable(minimum_ms) => {
                    // A persistent distributor refusal skips directly to level 3 (32 min).
                    // Store the level as well as the deadline so a failed trial reaches level 4.
                    entry.trip(now, minimum_ms, 3);
                }
                Outcome::Failure(minimum_ms) => {
                    entry.stable_since = None;
                    if !matches!(entry.phase, Phase::Closed) {
                        entry.trip(now, minimum_ms, 1);
                        continue;
                    }
                    while entry
                        .failures
                        .front()
                        .is_some_and(|at| now.saturating_sub(*at) > 60_000)
                    {
                        entry.failures.pop_front();
                    }
                    entry.failures.push_back(now);
                    if entry.failures.len() >= 3 || minimum_ms > 0 {
                        entry.trip(now, minimum_ms, 1);
                    }
                }
                Outcome::Success => {
                    entry.failures.clear();
                    if let Phase::Trial {
                        next,
                        successes,
                        lease,
                    } = &mut entry.phase
                    {
                        *successes += 1;
                        *lease = None;
                        *next = now.saturating_add(30_000);
                        if *successes >= 2 {
                            entry.phase = Phase::Closed;
                            entry.generation = entry.generation.wrapping_add(1);
                            entry.stable_since = Some(now);
                        }
                    } else {
                        let since = entry.stable_since.get_or_insert(now);
                        if now.saturating_sub(*since) >= 1_800_000 {
                            entry.level = 0;
                        }
                    }
                }
                Outcome::Neutral | Outcome::BalanceUnavailable(_) => {
                    if let Phase::Trial { next, lease, .. } = &mut entry.phase {
                        *lease = None;
                        *next = now.saturating_add(30_000);
                    }
                }
            }
        }
    }
    pub(crate) fn reports(&self) -> serde_json::Value {
        let now = self.now();
        serde_json::json!(self.entries.lock().iter().filter_map(|(key, e)| {
            let (status, retry_ms) = match e.phase {
                Phase::Closed => return None,
                Phase::Open(until) => ("cooling", until.saturating_sub(now)),
                Phase::Trial { next, lease, .. } => (if lease.is_some() { "trial_in_flight" } else { "trial_ready" }, next.saturating_sub(now)),
            };
            Some(serde_json::json!({"key":key,"status":status,"level":e.level,"retry_after_ms":retry_ms}))
        }).collect::<Vec<_>>())
    }
}
