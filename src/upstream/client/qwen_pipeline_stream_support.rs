//! Lazy public-router body polling stays inside the same test transport scope as request dispatch.
use super::super::*;
pub(super) fn delta(text: &str) -> String {
    frame(
        json!({"id":"qwen-stream","model":"qwen-synthetic","choices":[{"index":0,
        "delta":{"role":"assistant","content":text},"finish_reason":null}]}),
    )
}
pub(super) fn finish(reason: &str) -> String {
    frame(json!({"id":"qwen-stream","choices":[{"index":0,"delta":{},"finish_reason":reason}]}))
}
pub(super) fn usage() -> String {
    frame(
        json!({"choices":[],"usage":{"prompt_tokens":8,"completion_tokens":2,
        "total_tokens":10,"prompt_tokens_details":{"cached_tokens":3}}}),
    )
}
pub(super) fn frame(value: Value) -> String {
    format!("data: {value}\n\n")
}
pub(super) async fn open(fixture: &Fixture, endpoint: &str) -> axum::response::Response {
    let body = if endpoint == "responses" {
        json!({"model":"public-alias","input":"Reply OK","stream":true,"max_output_tokens":32})
    } else {
        json!({"model":"public-alias","messages":[{"role":"user","content":"Reply OK"}],
            "stream":true,"stream_options":{"include_usage":true},"max_tokens":32})
    };
    build_router(fixture.console.state.clone())
        .oneshot(
            Request::post(format!("/v1/{endpoint}"))
                .header("authorization", "Bearer public-api-contract-secret")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}
pub(super) async fn drain(mut body: Body) -> (String, bool) {
    let mut output = Vec::new();
    let mut failed = false;
    while let Some(frame) = body.frame().await {
        match frame {
            Ok(frame) => {
                if let Ok(bytes) = frame.into_data() {
                    output.extend_from_slice(&bytes);
                }
            }
            Err(_) => {
                failed = true;
                break;
            }
        }
    }
    (String::from_utf8(output).unwrap(), failed)
}
pub(super) async fn audit(fixture: &Fixture) -> db::GatewayRequestAuditView {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let rows = fixture
                .local
                .list_audits(&db::RequestAuditFilters::default())
                .await
                .unwrap();
            if rows.len() == 1
                && matches!(
                    rows[0].status.as_str(),
                    "completed" | "failed" | "cancelled"
                )
            {
                return rows.into_iter().next().unwrap();
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}
pub(super) fn assert_identity(server: &wire::Server, row: &db::GatewayRequestAuditView) {
    let calls = server.calls();
    assert_eq!(calls.len(), 1, "No hidden retry or channel change");
    assert_eq!(calls[0].authorization, "Bearer synthetic-exact-key");
    assert!(calls[0]
        .path
        .contains("workspace-b.ap-southeast-1.maas.aliyuncs.com"));
    assert!(calls[0].user_agent.contains("codex"));
    assert_eq!(calls[0].body["model"], "qwen-synthetic");
    assert_eq!(calls[0].body["stream"], true);
    assert_eq!(row.provider_account_id.as_deref(), Some("qwen-pipeline"));
    assert_eq!(
        row.route_trace.as_ref().unwrap()["realCredentialRef"],
        "exact"
    );
}
