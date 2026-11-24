//! Codex-specific native TLS and reqwest proxy defaults, separate from browser clients.
use std::time::Duration;

#[path = "native_codex_ca.rs"]
mod custom_ca;
#[path = "native_codex_redirect.rs"]
mod redirect;
pub(crate) use redirect::send;
#[path = "native_codex_pool.rs"]
mod pool;
pub(crate) use pool::NativeCodexConnection;
#[cfg(test)]
#[path = "native_codex_timeout_tests.rs"]
mod timeout_tests;

pub(crate) fn client(timeout: Duration) -> reqwest::Client {
    build(timeout, false, false, true)
}

pub(crate) fn websocket_client(timeout: Duration) -> reqwest::Client {
    build(timeout, true, false, true)
}

pub(crate) fn websocket_tls() -> Result<rustls::ClientConfig, &'static str> {
    custom_ca::websocket_tls()
}

fn build(timeout: Duration, websocket: bool, session: bool, codex_tls12: bool) -> reqwest::Client {
    // Use the same environment/system proxy discovery as the reference CLI;
    // do not mutate the process environment or reuse the browser emulation factory.
    let base =
        super::proxy_settings::reqwest(reqwest::Client::builder(), super::proxy_settings::active());
    let (builder, has_custom_ca) = custom_ca::configure(base, websocket)
        .expect("invalid native Codex custom CA configuration");
    let builder = if websocket {
        // The reference WS dialer leaves TCP_NODELAY at its socket default;
        // reqwest enables it by default. Keep this choice local to WS.
        builder.http1_only().tcp_nodelay(false)
    } else {
        // Evict idle pooled sockets before Cloudflare/relays silently half-close them (~30-60s).
        builder
            .pool_idle_timeout(Duration::from_secs(20))
            .tcp_keepalive(Duration::from_secs(15))
    };
    // Codex 0.155.1 locks native-tls 0.2.14: its Windows Schannel backend
    // offers at most TLS 1.2. Reproduce that ceiling only for official Codex/ChatGPT
    // profiles; generic third-party OpenAI relays frequently require TLS 1.3.
    #[cfg(windows)]
    let builder = if has_custom_ca || !codex_tls12 {
        builder
    } else {
        builder.max_tls_version(reqwest::tls::Version::TLS_1_2)
    };
    #[cfg(not(windows))]
    let _ = (has_custom_ca, codex_tls12);
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
    } else if crate::request_wait::transport::managed() {
        // Managed requests own the response deadline, including quiet SSE periods.
        builder.connect_timeout(timeout.min(Duration::from_secs(15)))
    } else {
        builder.connect_timeout(timeout).read_timeout(timeout)
    };
    builder
        .build()
        .expect("failed to build native Codex client")
}
