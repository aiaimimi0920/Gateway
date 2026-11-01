//! Final request audit must not turn a local 502 transport diagnostic into account cooldown.
use super::*;

#[tokio::test]
async fn native_network_diagnostics_and_budget_cancellation_never_persist_model_failure() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let error = crate::http_client::send_native_codex(
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .post(format!("http://{address}/")),
        "fixture",
    )
    .await
    .unwrap_err();
    assert_eq!(error.http_status, Some(502));
    assert_eq!(
        error.provider_response_status(),
        None,
        "a local 502 is not an upstream HTTP response"
    );
    assert!(!records_failure(&error));
    let budget =
        crate::pipeline::request_budget::RequestBudget::new(1, std::time::Duration::from_secs(30));
    assert!(!records_failure(&budget.cancel_caller()));
    assert_eq!(budget.cancel_caller().provider_response_status(), None);
    assert_eq!(
        crate::error::classify_upstream_error(500, "upstream failed", None)
            .provider_response_status(),
        Some(500)
    );
    assert!(records_failure(&crate::error::classify_upstream_error(
        401,
        "unauthorized",
        None
    )));
    assert!(records_failure(&crate::error::classify_upstream_error(
        500,
        "upstream failed",
        None
    )));
}
