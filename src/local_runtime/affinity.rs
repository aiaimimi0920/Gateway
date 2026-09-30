//! Advisory affinity can evict entries; durable authorization never lives in this cache.
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct AffinityCache(Mutex<HashMap<[u8; 32], (String, Instant)>>);

fn key(scope: &str, model: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(scope.len().to_le_bytes());
    digest.update(scope.as_bytes());
    digest.update(model.as_bytes());
    digest.finalize().into()
}

impl super::LocalRuntime {
    pub(crate) fn credential_affinity(&self, scope: &str, model: &str) -> Option<String> {
        self.affinity
            .0
            .lock()
            .get(&key(scope, model))
            .filter(|(_, expiry)| *expiry > Instant::now())
            .map(|(id, _)| id.clone())
    }

    pub(crate) fn set_credential_affinity(&self, scope: &str, model: &str, id: &str) {
        let mut cache = self.affinity.0.lock();
        let now = Instant::now();
        if cache.len() >= 4096 {
            cache.retain(|_, (_, expiry)| *expiry > now);
            if cache.len() >= 4096 {
                if let Some(oldest) = cache
                    .iter()
                    .min_by_key(|(_, (_, expiry))| *expiry)
                    .map(|(key, _)| *key)
                {
                    cache.remove(&oldest);
                }
            }
        }
        cache.insert(
            key(scope, model),
            (id.to_owned(), now + Duration::from_secs(3600)),
        );
    }
}
