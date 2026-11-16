//! Private synthetic fetches exercise the real management handler without upstream traffic.
use super::*;
use axum::http::{header::CACHE_CONTROL, Method, StatusCode};
use serde_json::json;

#[path = "../../../../tests/console_contract_support/mod.rs"]
mod support;
use support::{
    grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture,
    MANAGEMENT_TOKEN,
};

const BASE: &str = "https://workspace-a.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1";
fn fixture() -> ConsoleStateFixture {
    ConsoleStateFixture::new(serde_json::from_value(json!({"providers":[{
        "id":"arbitrary", "preset":"qwen-official-api", "base_url":BASE, "explicit_credentials_only":true,
        "credentials":[{"id":"a", "api_key":"synthetic-a"},
            {"id":"b", "base_url":BASE.replace("workspace-a","workspace-b"), "api_key":"synthetic-b", "enabled":false}]
    }]})).unwrap(), true)
}
fn body(value: serde_json::Value) -> CatalogRequest {
    serde_json::from_value(value).unwrap()
}
fn catalog(base: String) -> ModelCatalog {
    let endpoint = QwenEndpoint::parse(&base).unwrap();
    ModelCatalog {
        source_url: endpoint.catalog_source,
        account_base_url: Some(endpoint.base_url),
        supported_endpoints: Default::default(),
        checked_at: "2026-11-02T00:00:00Z".into(),
        models: vec!["qwen-synthetic".into()],
    }
}
async fn headers(fixture: &ConsoleStateFixture) -> HeaderMap {
    let grant = grant_console_secret_access(&fixture.state).await;
    HeaderMap::from_iter([
        (
            "x-management-token".parse().unwrap(),
            MANAGEMENT_TOKEN.parse().unwrap(),
        ),
        ("x-secret-grant".parse().unwrap(), grant.parse().unwrap()),
    ])
}
fn run<F: Future<Output = ()> + 'static>(test: fn() -> F) {
    // These handler cases share production discovery admission; their own runtimes must not race it.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = SERIAL.lock().unwrap();
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(test());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn qwen_management_catalog_uses_exact_stored_override_and_never_persists_metadata() {
    run(stored);
}
async fn stored() {
    let fixture = fixture();
    let headers = headers(&fixture).await;
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let response = read_with_fetch(
        fixture.state.clone(),
        None,
        None,
        headers,
        body(json!({"credentialId":"b"})),
        |base, key| async move {
            assert_eq!(base, BASE.replace("workspace-a", "workspace-b"));
            assert_eq!(key, "synthetic-b");
            Ok(catalog(base))
        },
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CACHE_CONTROL], "no-store");
    let result = parse_json(response).await;
    assert_eq!(result["revision"], revision);
    assert_eq!(
        result["catalog"]["accountBaseUrl"],
        BASE.replace("workspace-a", "workspace-b")
    );
    assert_eq!(
        result["catalog"]["sourceUrl"],
        "https://dashscope-intl.aliyuncs.com/api/v1/models"
    );
    assert!(!result.to_string().contains("synthetic-b"));
    let after = fixture.state.route_config.snapshot();
    assert_eq!(after.revision().id(), revision);
    assert!(after.document().providers[0].credentials[1]
        .model_catalog
        .is_none());
    let redacted = crate::console::secrets::redact_route_document(after.document()).unwrap();
    let mut document = serde_json::to_value(redacted.document).unwrap();
    document["providers"][0]["credentials"][1]["model_catalog"] = result["catalog"].clone();
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|secret| secret.configured)
        .map(|secret| json!({"path":secret.path,"operation":"keep"}))
        .collect();
    let grant = grant_console_secret_access(&fixture.state).await;
    let body = json!({"expectedRevision":revision,"document":document,"secretPatches":patches});
    let endpoint = "/v1/internal/gateway/console/route-config";
    let response = send_console_json(
        &fixture.state,
        Method::PUT,
        endpoint,
        body.clone(),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let saved = fixture.state.route_config.snapshot();
    assert_eq!(
        saved.document().providers[0].credentials[1]
            .model_catalog
            .as_ref()
            .unwrap()
            .account_base_url,
        Some(BASE.replace("workspace-a", "workspace-b"))
    );
    assert_eq!(
        saved.document().providers[0].credentials[1]
            .api_key
            .as_deref(),
        Some("synthetic-b")
    );
    assert_eq!(
        send_console_json(
            &fixture.state,
            Method::PUT,
            endpoint,
            body,
            Some(&grant),
            None
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

#[test]
fn qwen_management_catalog_new_key_uses_only_explicit_validated_draft_target() {
    run(draft);
}
async fn draft() {
    let fixture = fixture();
    let headers = headers(&fixture).await;
    let target = BASE
        .replace("workspace-a", "new-workspace")
        .replace("ap-southeast-1", "cn-hongkong");
    let expected = target.clone();
    let response = read_with_fetch(
        fixture.state.clone(),
        None,
        None,
        headers,
        body(json!({"providerId":"arbitrary", "accountBaseUrl":target, "apiKey":"synthetic-new"})),
        |base, key| async move {
            assert_eq!(base, expected);
            assert_eq!(key, "synthetic-new");
            Ok(catalog(base))
        },
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        parse_json(response).await["catalog"]["sourceUrl"],
        "https://cn-hongkong.dashscope.aliyuncs.com/api/v1/models"
    );
}

#[test]
fn qwen_management_catalog_rechecks_grant_after_fetch() {
    run(revoked);
}
async fn revoked() {
    let fixture = fixture();
    let headers = headers(&fixture).await;
    let state = fixture.state.clone();
    let request = console_request_context(&headers, None);
    let error = read_with_fetch(
        fixture.state.clone(),
        None,
        None,
        headers,
        body(json!({"credentialId":"a"})),
        |base, _| async move {
            state
                .console_auth
                .logout(&request, MANAGEMENT_TOKEN)
                .unwrap();
            Ok(catalog(base))
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.http_status, Some(403));
}

#[test]
fn qwen_management_catalog_rechecks_revision_and_shared_source_binding_after_fetch() {
    run(stale);
}
async fn stale() {
    let fixture = fixture();
    let headers = headers(&fixture).await;
    let state = fixture.state.clone();
    let error = read_with_fetch(
        fixture.state.clone(),
        None,
        None,
        headers.clone(),
        body(json!({"credentialId":"a"})),
        |base, _| async move {
            let mut document = state.route_config.snapshot().document().clone();
            document.providers[0].credentials[0].account_name = Some("changed".into());
            state.route_config.replace_document(document).unwrap();
            Ok(catalog(base))
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.http_status, Some(409));
    let error = read_with_fetch(
        fixture.state.clone(),
        None,
        None,
        headers,
        body(json!({"credentialId":"a"})),
        |base, _| async move { Ok(catalog(base.replace("workspace-a", "workspace-b"))) },
    )
    .await
    .unwrap_err();
    assert_eq!(error.http_status, Some(503));
}

#[test]
fn qwen_management_catalog_does_not_fetch_without_grant_and_releases_slot_on_cancel() {
    run(cancelled);
}
async fn cancelled() {
    let fixture = fixture();
    let denied_headers = HeaderMap::from_iter([(
        "x-management-token".parse().unwrap(),
        MANAGEMENT_TOKEN.parse().unwrap(),
    )]);
    let error = read_with_fetch(
        fixture.state.clone(),
        None,
        None,
        denied_headers,
        body(json!({"credentialId":"a"})),
        |_, _| async {
            panic!("must not fetch without a grant");
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.http_status, Some(403));
    let headers = headers(&fixture).await;
    let slots = &crate::provider_discovery::background::SLOTS;
    let available = slots.available_permits();
    let held = slots.try_acquire_many(available as u32).unwrap();
    let busy = read_with_fetch(
        fixture.state.clone(),
        None,
        None,
        headers.clone(),
        body(json!({"credentialId":"a"})),
        |_, _| async {
            panic!("busy admission must not fetch");
        },
    )
    .await
    .unwrap_err();
    assert_eq!(busy.http_status, Some(409));
    drop(held);
    let read = read_with_fetch(
        fixture.state.clone(),
        None,
        None,
        headers.clone(),
        body(json!({"credentialId":"a"})),
        |_, _| async { std::future::pending::<Result<ModelCatalog, GatewayError>>().await },
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(10), read)
            .await
            .is_err()
    );
    assert_eq!(slots.available_permits(), available);
    let response = read_with_fetch(
        fixture.state.clone(),
        None,
        None,
        headers,
        body(json!({"credentialId":"a"})),
        |base, _| async { Ok(catalog(base)) },
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
