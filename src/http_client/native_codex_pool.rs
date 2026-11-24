//! A lazy pool per accepted client connection. Retire failed transports for future sends, never replay them.
use crate::error::GatewayError;
use parking_lot::Mutex;
use std::{sync::Arc, time::Duration};

#[derive(Default)]
struct Pool {
    generation: u64,
    clients: [Option<reqwest::Client>; 4],
}

/// Request/response clones retain their transport when the connection's cached generation is retired.
/// No global cache, background task, cookies or credentials outlive the accepted connection owner.
#[derive(Clone, Default)]
pub(crate) struct NativeCodexConnection(Arc<Mutex<Pool>>);

impl NativeCodexConnection {
    #[cfg(test)]
    pub(crate) fn generation(&self) -> u64 {
        self.0.lock().generation
    }

    pub(crate) fn client(&self, timeout: Duration) -> (reqwest::Client, u64) {
        self.select(timeout, false)
    }

    pub(crate) fn session_client(&self, timeout: Duration) -> (reqwest::Client, u64) {
        self.select(timeout, true)
    }

    fn select(&self, timeout: Duration, session: bool) -> (reqwest::Client, u64) {
        let slot =
            usize::from(crate::request_wait::transport::managed()) * 2 + usize::from(session);
        let mut pool = self.0.lock();
        let client = pool.clients[slot]
            .get_or_insert_with(|| super::build(timeout, false, session))
            .clone();
        // Capture the generation under the same lock as client selection.
        (client, pool.generation)
    }

    pub(crate) fn recover_after_send(&self, generation: u64, error: &GatewayError) -> bool {
        // This is transport retirement, not a retry permission. Certificates/auth must not be bypassed.
        let Some(evidence) = error.native_transport.as_ref() else {
            return false;
        };
        if matches!(
            evidence.category(),
            "tls_certificate" | "proxy_authentication"
        ) {
            return false;
        }
        let mut pool = self.0.lock();
        // A late error from an older send cannot keep discarding a newly rebuilt healthy pool.
        if pool.generation != generation {
            return false;
        }
        pool.clients = Default::default();
        pool.generation = pool.generation.wrapping_add(1);
        tracing::debug!(
            category = evidence.category(),
            generation = pool.generation,
            "Retired native Codex connection transports for future requests"
        );
        true
    }
}

#[cfg(test)]
#[path = "native_codex_pool_tests.rs"]
mod tests;
