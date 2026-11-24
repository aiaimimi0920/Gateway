//! Request-scoped loopback substitution; absent fixture scope fails before any vendor request.
tokio::task_local! { static ENDPOINT: String; }
pub(crate) async fn scope<T>(endpoint: String, future: impl std::future::Future<Output = T>) -> T {
    let url = url::Url::parse(&endpoint).unwrap();
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert!(url.port().is_some());
    assert_eq!(url.path(), "/mcp");
    ENDPOINT.scope(endpoint, future).await
}
pub(super) fn endpoint(expected: &str) -> String {
    assert_eq!(expected, crate::protocol::luma_oauth::RESOURCE);
    ENDPOINT
        .try_with(Clone::clone)
        .expect("MCP test requires a scoped loopback peer")
}
