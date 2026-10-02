#[path = "client_debug.rs"]
mod debug;

use super::{Duration, Emulation, PgPool, RedisPool, RequestTimeBrowserPolicy, UpstreamClient};

impl UpstreamClient {
    /// Create a new client with the given request timeout.
    ///
    /// Uses the configured Chrome TLS profile through wreq's BoringSSL backend.
    /// This alone does not establish complete JA3/JA4 identity or provider acceptance.
    pub fn new(timeout_secs: u64) -> Self {
        Self::new_with_runtime(timeout_secs, None, None)
    }

    pub fn new_with_runtime(
        timeout_secs: u64,
        redis_pool: Option<RedisPool>,
        pg_pool: Option<PgPool>,
    ) -> Self {
        let http = crate::http_client::builder()
            .emulation(Emulation::Chrome136)
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .expect("failed to build rquest client");
        let plain_http = crate::http_client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .expect("failed to build plain rquest client");
        let browser_executor_base_url = std::env::var("GATEWAY_BROWSER_EXECUTOR_BASE_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty());
        let browser_executor_bearer_token = std::env::var("GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let request_time_browser_policy = RequestTimeBrowserPolicy::from_env();

        Self {
            http,
            plain_http,
            freebuff: std::sync::Arc::new(crate::protocol::freebuff::RunRuntime::default()),
            timeout: Duration::from_secs(timeout_secs),
            browser_executor_base_url,
            browser_executor_bearer_token,
            request_time_browser_policy,
            redis_pool,
            pg_pool,
        }
    }
}
