//! Reserve room for a possible 402 *before* admitting a metered key. Healthy
//! keys leave no record once their last in-flight request finishes or is cancelled.
use super::{paused, unavailable, GatewayError, Instant, Key, RECHECK};
use parking_lot::Mutex;
use std::{collections::HashMap, future::Future};

pub(super) const CAPACITY: usize = 4096;

#[derive(Default, Debug)]
pub(super) struct Ledger(Mutex<HashMap<Key, Entry>>);

#[derive(Default, Debug)]
struct Entry {
    active: usize,
    next_probe: Option<Instant>,
    revision: u64,
}

pub(crate) struct Permit<'a> {
    ledger: &'a Ledger,
    key: Key,
}

impl Ledger {
    pub(super) async fn admit<F: Future<Output = Option<f64>>>(
        &self,
        key: Key,
        now: Instant,
        read: impl FnOnce() -> F,
    ) -> Result<Permit<'_>, GatewayError> {
        let revision = {
            let mut entries = self.0.lock();
            if entries.len() >= CAPACITY && !entries.contains_key(&key) {
                return Err(unavailable());
            }
            let entry = entries.entry(key).or_default();
            match entry.next_probe {
                None => {
                    entry.active += 1;
                    return Ok(Permit { ledger: self, key });
                }
                Some(at) if now < at => return Err(paused()),
                Some(_) => {
                    entry.next_probe = Some(now + RECHECK);
                    entry.revision += 1;
                    entry.revision
                }
            }
        };
        // A newer 402 or a later probe wins over an old positive balance reply.
        if read().await.is_some_and(|balance| balance > 0.0) {
            let mut entries = self.0.lock();
            if let Some(entry) = entries
                .get_mut(&key)
                .filter(|e| e.revision == revision && e.next_probe == Some(now + RECHECK))
            {
                entry.next_probe = None;
                entry.active += 1;
                return Ok(Permit { ledger: self, key });
            }
        }
        Err(paused())
    }
}

impl Permit<'_> {
    pub(super) fn pause(&self, now: Instant) {
        let mut entries = self.ledger.0.lock();
        let entry = entries.get_mut(&self.key).expect("live metered permit");
        entry.revision += 1;
        entry.next_probe = Some(now + RECHECK);
    }
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        let mut entries = self.ledger.0.lock();
        let entry = entries.get_mut(&self.key).expect("live metered permit");
        entry.active -= 1;
        if entry.active == 0 && entry.next_probe.is_none() {
            entries.remove(&self.key);
        }
    }
}
