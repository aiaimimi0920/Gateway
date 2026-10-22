//! Credential/session scoped, bounded date pins; no credential material is stored in the map.
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

pub fn pinned_date(seed: &str, session: &str) -> String {
    static PINS: OnceLock<Mutex<HashMap<[u8; 32], (String, Instant)>>> = OnceLock::new();
    let mut hash = Sha256::new();
    hash.update(seed);
    hash.update([0]);
    hash.update(session);
    let key: [u8; 32] = hash.finalize().into();
    let now = Instant::now();
    let mut pins = PINS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    pins.retain(|_, (_, last)| now.duration_since(*last) < Duration::from_secs(86400));
    if let Some((date, last)) = pins.get_mut(&key) {
        *last = now;
        return date.clone();
    }
    if pins.len() >= 4096 {
        if let Some(oldest) = pins
            .iter()
            .min_by_key(|(_, (_, last))| *last)
            .map(|(key, _)| *key)
        {
            pins.remove(&oldest);
        }
    }
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    pins.insert(key, (date.clone(), now));
    date
}
