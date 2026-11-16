//! 真实管理 router：目录读取不写入，普通 CAS 保存稳定账号与秘密。
use super::*;
use axum::http::Method;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};
const CONFIG: &str = "/v1/internal/gateway/console/route-config";

fn fixture() -> ConsoleStateFixture {
    ConsoleStateFixture::new(
        serde_json::from_value(json!({"providers":[{
        "id":"arbitrary", "preset":"minimax-m-plan", "base_url":super::super::BASE,
        "explicit_credentials_only":true, "credentials":[
            {"id":"stable", "api_key":KEY},
            {"id":"other", "api_key":"other-synthetic", "enabled":false}]}]}))
        .unwrap(),
        true,
    )
}
fn draft(f: &ConsoleStateFixture) -> Value {
    let snapshot = f.state.route_config.snapshot();
    let redacted = crate::console::secrets::redact_route_document(snapshot.document()).unwrap();
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| json!({"path":s.path,"operation":"keep"}))
        .collect();
    json!({"expectedRevision":snapshot.revision().id(), "document":redacted.document, "secretPatches":patches})
}
fn run<F: std::future::Future<Output = ()> + 'static>(f: fn() -> F) {
    // 管理 router 的大 future 使用固定测试线程栈，避免依赖默认线程栈大小。
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(f());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn minimax_management_keep_cannot_rebind_key_to_another_product() {
    run(keep_binding);
}
async fn keep_binding() {
    let fixture = fixture();
    let original = draft(&fixture);
    let mut body = original.clone();
    body["document"]["providers"][0]["preset"] = json!(super::super::PRESET_CN);
    body["document"]["providers"][0]["base_url"] = json!(super::super::BASE_CN);
    let response = send_console_json(&fixture.state, Method::PUT, CONFIG, body, None, None).await;
    let status = response.status();
    let value = parse_json(response).await;
    assert!(!status.is_success());
    assert!(
        value
            .to_string()
            .contains("secret_keep_destination_changed"),
        "{value}"
    );
    assert_eq!(draft(&fixture), original);
}

#[test]
fn minimax_management_creates_and_replaces_key_without_changing_identity() {
    run(create_and_replace);
}
async fn create_and_replace() {
    let fixture = fixture();
    let grant = grant_console_secret_access(&fixture.state).await;
    let mut body = draft(&fixture);
    body["document"]["providers"][0]["credentials"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"created", "account_name":"First", "supported_models":[UPSTREAM_MODEL]}));
    body["secretPatches"].as_array_mut().unwrap().push(json!({
        "path":"/providers/0/credentials/2/api_key", "operation":"replace", "value":"created-synthetic"
    }));
    for (name, key) in [
        ("First", "created-synthetic"),
        ("Renamed", "replacement-synthetic"),
    ] {
        if name == "Renamed" {
            body = draft(&fixture);
            body["document"]["providers"][0]["credentials"][2]["account_name"] = json!(name);
            let patches = body["secretPatches"].as_array_mut().unwrap();
            patches.retain(|p| p["path"] != "/providers/0/credentials/2/api_key");
            patches.push(json!({"path":"/providers/0/credentials/2/api_key", "operation":"replace", "value":key}));
        }
        let denied = send_console_json(
            &fixture.state,
            Method::PUT,
            CONFIG,
            body.clone(),
            None,
            None,
        )
        .await;
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        let response = send_console_json(
            &fixture.state,
            Method::PUT,
            CONFIG,
            body.clone(),
            Some(&grant),
            None,
        )
        .await;
        let status = response.status();
        let result = parse_json(response).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert!(!result.to_string().contains(key));
        assert_eq!(
            send_console_json(
                &fixture.state,
                Method::PUT,
                CONFIG,
                body.clone(),
                Some(&grant),
                None
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
        let snapshot = fixture.state.route_config.snapshot();
        let accounts = &snapshot.document().providers[0].credentials;
        assert_eq!(accounts.len(), 3);
        assert_eq!(accounts[0].api_key.as_deref(), Some(KEY));
        assert_eq!(accounts[2].id.as_deref(), Some("created"));
        assert_eq!(accounts[2].account_name.as_deref(), Some(name));
        assert_eq!(accounts[2].api_key.as_deref(), Some(key));
        assert!(accounts[2].refresh_token.is_none());
    }
}
