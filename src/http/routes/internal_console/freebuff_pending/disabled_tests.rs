//! Disabled implementation line refuses both operations before any pending identity resolution.
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
use axum::http::{Method, StatusCode};
use serde_json::json;
#[tokio::test]
async fn freebuff_pending_management_compiled_out_rejects_read_and_decision() {
    let f = support::ConsoleStateFixture::new(
        serde_json::from_value(
            json!({"providers":[{"id":"pool","preset":"freebuff","base_url":"https://freebuff.com",
        "explicit_credentials_only":true,"credentials":[{"id":"b","api_key":"synthetic-b",
        "extra_body":{"freebuffBusiness":{"origin":"https://codebuff.com","client":"cli"}}}]}]}),
        )
        .unwrap(),
        true,
    );
    let grant = support::grant_console_secret_access(&f.state).await;
    for decision in [false, true] {
        let mut body = json!({"providerId":"pool","credentialId":"b","expectedRevision":f.state.route_config.snapshot().revision().id()});
        let path = if decision {
            body["pendingId"] = json!(uuid::Uuid::new_v4().to_string());
            body["quoteId"] = json!(uuid::Uuid::new_v4().to_string());
            body["decision"] = json!("approve");
            "/v1/internal/gateway/console/freebuff-pending-purchases/decision"
        } else {
            "/v1/internal/gateway/console/freebuff-pending-purchases"
        };
        let response =
            support::send_console_json(&f.state, Method::POST, path, body, Some(&grant), None)
                .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(
            support::parse_json(response).await["error"]["code"],
            "gateway_provider_line_compiled_out"
        );
    }
}
