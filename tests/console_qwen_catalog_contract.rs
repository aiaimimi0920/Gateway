//! Real management router negative contracts never contact Qwen or read real credentials.
#[path = "console_contract_support/mod.rs"]
mod support;
use axum::http::{Method, StatusCode};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::json;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};

const ENDPOINT: &str = "/v1/internal/gateway/console/qwen-model-catalog";
const BASE: &str = "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";

async fn contract() {
    let active: RouteConfigYaml = serde_json::from_value(json!({"providers":[
        {"id":"ordinary", "preset":"qwen-official-api", "base_url":BASE,
         "credentials":[{"id":"a","api_key":""}]},
        {"id":"plan", "preset":"qwen-coding-plan-openai", "base_url":"https://coding.dashscope.aliyuncs.com/v1",
         "explicit_credentials_only":true,"credentials":[]},
        {"id":"web", "preset":"qwen-web-chat", "base_url":"https://chat.qwen.ai", "explicit_credentials_only":true,"credentials":[]}
    ]})).unwrap();
    let fixture = ConsoleStateFixture::new(active, true);
    let original = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let denied = send_console_json(
        &fixture.state,
        Method::POST,
        ENDPOINT,
        json!({"credentialId":"a"}),
        None,
        None,
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let grant = grant_console_secret_access(&fixture.state).await;
    for body in [
        json!({"credentialId":"a"}),
        json!({"credentialId":"missing"}),
        json!({"providerId":"plan","apiKey":"synthetic-key","accountBaseUrl":BASE}),
        json!({"providerId":"web","apiKey":"synthetic-key","accountBaseUrl":BASE}),
        json!({"providerId":"ordinary","apiKey":"synthetic-key","accountBaseUrl":"https://evil.invalid/v1"}),
        json!({"providerId":"ordinary","apiKey":"synthetic-key","accountBaseUrl":BASE.replace("workspace-a","trial")}),
        json!({"providerId":"ordinary","apiKey":"bad\r\nkey","accountBaseUrl":BASE}),
    ] {
        let response = send_console_json(
            &fixture.state,
            Method::POST,
            ENDPOINT,
            body,
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!parse_json(response)
            .await
            .to_string()
            .contains("synthetic-key"));
    }
    for body in [
        json!({}),
        json!({"providerId":"ordinary","apiKey":"synthetic-key"}),
        json!({"credentialId":"a","apiKey":"synthetic-key"}),
        json!({"credentialId":"a","accountBaseUrl":BASE}),
        json!({"credentialId":"a","sourceUrl":"https://evil.invalid/models"}),
        json!({"providerId":"ordinary","apiKey":"synthetic-key","accountBaseUrl":BASE,"sourceUrl":"https://evil.invalid/models"}),
    ] {
        let response = send_console_json(
            &fixture.state,
            Method::POST,
            ENDPOINT,
            body,
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        original
    );
}

#[test]
#[cfg(feature = "line-qwen-official-api")]
fn qwen_catalog_enforces_management_grant_product_and_nonarbitrary_target() {
    // The existing router future requires a larger Windows test stack.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(contract());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
#[cfg(not(feature = "line-qwen-official-api"))]
fn qwen_catalog_compiled_out_never_becomes_a_generic_directory() {
    std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            let fixture = ConsoleStateFixture::new(serde_json::from_value(json!({"providers":[]})).unwrap(), true);
            let grant = grant_console_secret_access(&fixture.state).await;
            let response = send_console_json(&fixture.state, Method::POST, ENDPOINT,
                json!({"providerId":"missing","apiKey":"synthetic-key","accountBaseUrl":BASE}), Some(&grant), None).await;
            assert_eq!(response.status(), StatusCode::CONFLICT);
            assert_eq!(parse_json(response).await["error"]["code"], "gateway_provider_line_compiled_out");
        });
    }).unwrap().join().unwrap();
}
