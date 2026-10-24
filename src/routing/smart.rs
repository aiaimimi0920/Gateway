//! Session-local stickiness and bounded model-specific availability, independent of authorization.
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Availability,
    Intelligence,
    Price,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "Vec<Dimension>")]
pub struct Order(pub Vec<Dimension>);
impl TryFrom<Vec<Dimension>> for Order {
    type Error = &'static str;
    fn try_from(value: Vec<Dimension>) -> Result<Self, Self::Error> {
        if value.len() != 3
            || [
                Dimension::Availability,
                Dimension::Intelligence,
                Dimension::Price,
            ]
            .iter()
            .any(|dimension| value.iter().filter(|v| *v == dimension).count() != 1)
        {
            return Err(
                "routing order must contain availability, intelligence and price exactly once",
            );
        }
        Ok(Self(value))
    }
}
impl Default for Order {
    fn default() -> Self {
        Self(vec![
            Dimension::Availability,
            Dimension::Intelligence,
            Dimension::Price,
        ])
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub order: Order,
    pub measurements: Vec<Measurement>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    pub provider: String,
    pub model: String,
    pub intelligence: Option<f64>,
    pub valid_until_unix: Option<i64>,
    pub input_per_million: Option<f64>,
    pub output_per_million: Option<f64>,
}
impl Policy {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.measurements.len() > 4096 {
            return Err("too many model measurements");
        }
        for row in &self.measurements {
            if row.provider.len() > 256
                || row.model.len() > 256
                || row.provider.is_empty()
                || row.model.is_empty()
                || row
                    .intelligence
                    .is_some_and(|v| !v.is_finite() || !(0.0..=100.0).contains(&v))
                || [row.input_per_million, row.output_per_million]
                    .into_iter()
                    .flatten()
                    .any(|v| !v.is_finite() || v < 0.0)
            {
                return Err("invalid model measurement");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct SmartRouting {
    policy: std::sync::OnceLock<Result<Policy, String>>,
    availability: Mutex<HashMap<(String, String), (f64, Instant)>>,
    sticky: Mutex<HashMap<String, (String, String, Instant, uuid::Uuid)>>,
}
const LIMIT: usize = 4096;
impl SmartRouting {
    pub fn policy(&self) -> Result<&Policy, String> {
        self.policy
            .get_or_init(|| {
                let settings = gateway_local_data::startup::StartupConfig::load()
                    .map_err(|e| e.to_string())?;
                let policy: Policy = settings
                    .routing_policy
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|e| e.to_string())?
                    .unwrap_or_default();
                policy.validate().map_err(str::to_owned)?;
                Ok(policy)
            })
            .as_ref()
            .map_err(Clone::clone)
    }
    pub fn begin(&self, scope: Option<&str>, model: &str, provider: &str) {
        self.begin_request(scope, model, provider, uuid::Uuid::nil());
    }
    pub fn begin_request(
        &self,
        scope: Option<&str>,
        model: &str,
        provider: &str,
        request: uuid::Uuid,
    ) {
        let Some(scope) = scope else { return };
        let mut map = self.sticky.lock();
        if map.len() >= LIMIT && !map.contains_key(scope) {
            if let Some(key) = map.iter().min_by_key(|(_, v)| v.2).map(|(k, _)| k.clone()) {
                map.remove(&key);
            }
        }
        map.insert(
            scope.to_owned(),
            (
                model.to_owned(),
                provider.to_owned(),
                Instant::now(),
                request,
            ),
        );
    }
    pub fn sticky(&self, scope: Option<&str>, model: &str) -> Option<String> {
        let map = self.sticky.lock();
        let (last_model, provider, at, _) = map.get(scope?)?;
        (last_model == model && at.elapsed() < Duration::from_secs(3600)).then(|| provider.clone())
    }
    pub fn score(&self, provider: &str, model: &str) -> f64 {
        self.availability
            .lock()
            .get(&(provider.into(), model.into()))
            .map(|(value, at)| recovered(*value, at.elapsed()))
            .unwrap_or(100.0)
    }
    /// Only terminal upstream failure releases stickiness, never an intermediate retry.
    /// A late completion must not evict a newer request, even to the same provider.
    pub fn release_failed_request(
        &self,
        scope: Option<&str>,
        model: &str,
        provider: &str,
        request: uuid::Uuid,
    ) {
        let Some(scope) = scope else { return };
        let mut map = self.sticky.lock();
        if map
            .get(scope)
            .is_some_and(|(m, p, _, r)| m == model && p == provider && *r == request)
        {
            map.remove(scope);
        }
    }
    pub fn outcome(&self, provider: &str, model: &str, success: bool) {
        let mut map = self.availability.lock();
        let key = (provider.to_owned(), model.to_owned());
        if map.len() >= LIMIT && !map.contains_key(&key) {
            if let Some(old) = map.iter().min_by_key(|(_, v)| v.1).map(|(k, _)| k.clone()) {
                map.remove(&old);
            }
        }
        let score = map
            .get(&key)
            .map(|(v, at)| recovered(*v, at.elapsed()))
            .unwrap_or(100.0);
        map.insert(
            key,
            (
                (score + if success { 5.0 } else { -15.0 }).clamp(0.0, 100.0),
                Instant::now(),
            ),
        );
    }
}
fn recovered(score: f64, elapsed: Duration) -> f64 {
    // One point per minute, lazy evaluation: no per-candidate timer/task.
    (score + elapsed.as_secs_f64() / 60.0).min(100.0)
}
pub fn tier(score: f64) -> u8 {
    (score.clamp(0.0, 100.0) / 25.0).floor().min(3.0) as u8
}
