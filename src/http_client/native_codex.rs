//! Codex-specific native TLS and reqwest proxy defaults, separate from browser clients.
use std::time::Duration;

#[path = "native_codex_ca.rs"]
mod custom_ca;
#[path = "native_codex_redirect.rs"]
mod redirect;
#[cfg(test)]
#[path = "native_codex_timeout_tests.rs"]
mod timeout_tests;

/// A pool belongs to one accepted client connection, not the whole Gateway.
/// Lazy initialization avoids creating native clients for unrelated routes.
/// Dropping the connection service releases its pool; no global cache or task
/// outlives that owner. In-flight request builders retain their own client clone.
#[derive(Clone, Default)]
pub(crate) struct NativeCodexConnection(
    std::sync::Arc<std::sync::OnceLock<reqwest::Client>>,
    std::sync::Arc<std::sync::OnceLock<reqwest::Client>>,
);

impl NativeCodexConnection {
    pub(crate) fn client(&self, timeout: Duration) -> &reqwest::Client {
        self.0.get_or_init(|| client(timeout))
    }

    pub(crate) fn session_client(&self, timeout: Duration) -> &reqwest::Client {
        self.1.get_or_init(|| build(timeout, false, true))
    }
}

pub(crate) fn client(timeout: Duration) -> reqwest::Client {
    build(timeout, false, false)
}

pub(crate) fn websocket_client(timeout: Duration) -> reqwest::Client {
    build(timeout, true, false)
}

pub(crate) fn websocket_tls() -> Result<rustls::ClientConfig, &'static str> {
    custom_ca::websocket_tls()
}

fn build(timeout: Duration, websocket: bool, session: bool) -> reqwest::Client {
    // Use the same environment/system proxy discovery as the reference CLI;
    // do not mutate the process environment or reuse the browser emulation factory.
    let (builder, has_custom_ca) = custom_ca::configure(reqwest::Client::builder(), websocket)
        .expect("invalid native Codex custom CA configuration");
    let builder = if websocket {
        // The reference WS dialer leaves TCP_NODELAY at its socket default;
        // reqwest enables it by default. Keep this choice local to WS.
        builder.http1_only().tcp_nodelay(false)
    } else {
        builder
    };
    // Codex 0.155.1 locks native-tls 0.2.14: its Windows Schannel backend
    // offers at most TLS 1.2. Reproduce that ceiling only in this native lane;
    // retain certificate validation and the maintained TLS dependency.
    #[cfg(windows)]
    let builder = if has_custom_ca {
        builder
    } else {
        builder.max_tls_version(reqwest::tls::Version::TLS_1_2)
    };
    #[cfg(not(windows))]
    let _ = has_custom_ca;
    let builder = builder
        // Cookie sessions never replay manually attached cookies on a redirect.
        .redirect(if session {
            reqwest::redirect::Policy::none()
        } else {
            redirect::policy()
        });
    // A Responses stream may remain productive longer than the request budget.
    // Bound connection/idle reads, not its total lifetime. WS keeps its dial limit.
    let builder = if websocket {
        builder.timeout(timeout)
    } else {
        builder.connect_timeout(timeout).read_timeout(timeout)
    };
    builder
        .build()
        .expect("failed to build native Codex client")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_pool_is_lazy_shared_within_connection_and_distinct_between_connections() {
        let first = NativeCodexConnection::default();
        let same = first.clone();
        let second = NativeCodexConnection::default();
        assert!(first.0.get().is_none());
        assert!(second.0.get().is_none());
        assert!(std::ptr::eq(
            first.client(Duration::from_secs(5)),
            same.client(Duration::from_secs(5))
        ));
        assert!(!std::ptr::eq(
            first.client(Duration::from_secs(5)),
            second.client(Duration::from_secs(5))
        ));
        let weak = std::sync::Arc::downgrade(&first.0);
        drop(first);
        assert!(weak.upgrade().is_some());
        drop(same);
        assert!(weak.upgrade().is_none());
    }
}
