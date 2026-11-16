//! Compiled-out management must refuse before account lookup, storage or any network access.
#[path = "../../../../tests/console_contract_support/mod.rs"]
mod support;

#[tokio::test]
async fn freebuff_device_management_compiled_out_precedes_storage_and_network() {
    let f = support::ConsoleStateFixture::new(serde_yaml::from_str("providers: []").unwrap(), true);
    let grant = support::grant_console_secret_access(&f.state).await;
    for endpoint in [
        "/v1/internal/gateway/console/freebuff-device-registration",
        "/v1/internal/gateway/console/freebuff-device-registration/status",
    ] {
        let response = support::send_console_json(&f.state, axum::http::Method::POST, endpoint,
        serde_json::json!({"providerId":"none","credentialId":"none","expectedRevision":f.state.route_config.snapshot().revision().id(),
            "businessOrigin":"https://codebuff.com","client":"cli"}), Some(&grant), None).await;
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
        assert_eq!(
            support::parse_json(response).await["error"]["code"],
            "gateway_provider_line_compiled_out"
        );
    }
}
