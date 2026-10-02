//! 客户端诊断只暴露非敏感状态，不要求底层 HTTP 客户端实现 Debug。
use super::UpstreamClient;

impl std::fmt::Debug for UpstreamClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpstreamClient")
            .field("timeout", &self.timeout)
            .field(
                "request_time_browser_policy",
                &self.request_time_browser_policy,
            )
            .field(
                "browser_executor_configured",
                &self.browser_executor_base_url.is_some(),
            )
            .field("redis_configured", &self.redis_pool.is_some())
            .field("database_configured", &self.pg_pool.is_some())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn diagnostics_exclude_executor_address_token_and_http_clients() {
        let mut client = UpstreamClient::new(9);
        client.browser_executor_base_url =
            Some("http://fixture.invalid/?token=fixture-endpoint-secret".into());
        client.browser_executor_bearer_token = Some("fixture-bearer-secret".into());
        let debug = format!("{client:?}");
        assert!(debug.contains("browser_executor_configured: true"));
        assert!(debug.contains("timeout"));
        for secret in [
            "fixture.invalid",
            "fixture-endpoint-secret",
            "fixture-bearer-secret",
            "plain_http",
        ] {
            assert!(
                !debug.contains(secret),
                "client diagnostics must omit sensitive owner state"
            );
        }
    }
}
