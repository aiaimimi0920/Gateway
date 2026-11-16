//! Compiled-out management routes retain authority checks and never reach storage/network.
use super::*;
use axum::http::{Method, StatusCode};
use serde_json::json;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
#[tokio::test]
async fn freebuff_claim_management_compiled_out_refuses_read_and_end() {
    let f = support::ConsoleStateFixture::new(serde_json::from_value(json!({"providers":[{"id":"pool","preset":"freebuff","base_url":"https://freebuff.com",
        "explicit_credentials_only":true,"credentials":[{"id":"b","api_key":"synthetic-b","extra_body":{"freebuffBusiness":{"origin":"https://codebuff.com","client":"cli"}}}]}]})).unwrap(),true);
    let grant = support::grant_console_secret_access(&f.state).await;
    for end in [false, true] {
        let path = if end {
            "/v1/internal/gateway/console/freebuff-session-claim/end"
        } else {
            "/v1/internal/gateway/console/freebuff-session-claim"
        };
        let mut body = json!({"providerId":"pool","credentialId":"b","expectedRevision":f.state.route_config.snapshot().revision().id()});
        if end {
            body["attemptId"] = json!(uuid::Uuid::new_v4().to_string());
        }
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
