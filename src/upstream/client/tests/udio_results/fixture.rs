//! A real loopback remote executor; no browser, env mutation or actual platform traffic.
use super::*;
use axum::{routing::post, Json, Router};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

pub fn complete(id: &str) -> Value {
    json!({"id":id,"status":"complete","finished":true,"song_path":"https://asset.invalid/not-audio"})
}
pub async fn invoke(
    result: Value,
    wait: bool,
    endpoint: EndpointKind,
) -> Result<Value, GatewayError> {
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let app = Router::new().route(
        "/v1/internal/browser-executor/execute",
        post(move || {
            observed.fetch_add(1, Ordering::SeqCst);
            let result = result.clone();
            async move { Json(json!({"ok":true,"result":result})) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut client = UpstreamClient::new(1);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    client.browser_executor_base_url = Some(base);
    client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
    let mut payload = make_payload("udio_compatible", "https://www.udio.com");
    payload
        .headers
        .insert("Cookie".into(), "fixture=not-real".into());
    let mut request = make_request(ProtocolFamily::OpenAi, endpoint);
    request.raw_body = json!({"prompt":"fixture","wait_audio":wait});
    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.execute_udio_media("fixture", &payload, &request, "fixture", None),
    )
    .await;
    server.abort();
    let _ = server.await;
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    outcome.expect("bounded remote metadata call")
}
pub fn assert_rejected(result: Result<Value, GatewayError>) {
    let error =
        result.expect_err("a malformed worker success cannot become a public media success");
    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    assert!(!error.retryable);
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::Abort { .. }
    ));
    assert!(!error.message.contains("private-result"));
}
