//! Per-runtime advisory latency, never an authorization or health-state owner.
//! Bounded keys and an expiry index keep memory and lock work predictable.
use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use super::candidate::RouteCandidate;
use crate::protocol::canonical::CanonicalRelayRequest;

const CAPACITY: usize = 4_096;
const TTL: Duration = Duration::from_secs(600);
const ALPHA: f64 = 0.3;
/// Cold/expired candidates are neutral, not a fictitious zero-latency winner.
pub const NEUTRAL_LATENCY_MS: f64 = 1_000.0;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LatencyKey {
    subject: String,
    model: String,
    profile: String,
    adapter: String,
    endpoint: u8,
    streaming: bool,
}

impl LatencyKey {
    pub fn for_request(candidate: &RouteCandidate, req: &CanonicalRelayRequest) -> Option<Self> {
        let model = candidate
            .upstream_model
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                crate::upstream::upstream_model_helpers::resolve_model(&candidate.payload, req)
            });
        let parts = [
            candidate.runtime_subject_id(),
            model,
            candidate.protocol_profile.as_str(),
            candidate.adapter.as_str(),
        ];
        // Reject oversized identities, rather than truncating into collisions.
        if parts.iter().any(|part| part.len() > 256) {
            return None;
        }
        Some(Self {
            subject: parts[0].into(),
            model: model.into(),
            profile: parts[2].into(),
            adapter: parts[3].into(),
            endpoint: req.endpoint_kind as u8,
            streaming: req.stream,
        })
    }
}

struct Sample {
    ewma_ms: f64,
    updated_at: Instant,
}

#[derive(Default)]
struct Cache {
    samples: HashMap<LatencyKey, Sample>,
    oldest: BTreeSet<(Instant, LatencyKey)>,
}

pub struct RoutingLatency {
    cache: Mutex<Cache>,
    capacity: usize,
    ttl: Duration,
}

impl Default for RoutingLatency {
    fn default() -> Self {
        Self {
            cache: Mutex::new(Cache::default()),
            capacity: CAPACITY,
            ttl: TTL,
        }
    }
}

impl RoutingLatency {
    pub fn record(&self, key: LatencyKey, elapsed: Duration) {
        self.record_at(key, elapsed, Instant::now());
    }

    pub fn estimate_ms(&self, key: &LatencyKey) -> Option<f64> {
        self.estimate_at(key, Instant::now())
    }

    fn record_at(&self, key: LatencyKey, elapsed: Duration, now: Instant) {
        // Finite, nonzero samples even on sub-ms clocks or enormous durations.
        let ms = elapsed.as_millis().clamp(1, 86_400_000) as f64;
        let mut cache = self.cache.lock();
        while cache
            .oldest
            .first()
            .is_some_and(|(at, _)| now.saturating_duration_since(*at) >= self.ttl)
        {
            evict_oldest(&mut cache);
        }
        let previous = cache.samples.remove(&key);
        let ewma_ms = if let Some(previous) = previous {
            cache.oldest.remove(&(previous.updated_at, key.clone()));
            ALPHA * ms + (1.0 - ALPHA) * previous.ewma_ms
        } else {
            ms
        };
        while cache.samples.len() >= self.capacity {
            evict_oldest(&mut cache);
        }
        cache.oldest.insert((now, key.clone()));
        cache.samples.insert(
            key,
            Sample {
                ewma_ms,
                updated_at: now,
            },
        );
    }

    fn estimate_at(&self, key: &LatencyKey, now: Instant) -> Option<f64> {
        let mut cache = self.cache.lock();
        let sample = cache.samples.get(key)?;
        if now.saturating_duration_since(sample.updated_at) < self.ttl {
            return Some(sample.ewma_ms);
        }
        let at = sample.updated_at;
        cache.oldest.remove(&(at, key.clone()));
        cache.samples.remove(key);
        None
    }
}

fn evict_oldest(cache: &mut Cache) {
    if let Some((_, key)) = cache.oldest.pop_first() {
        cache.samples.remove(&key);
    }
}

#[cfg(test)]
mod tests;
