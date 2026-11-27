//! Import-only secrets cannot be repurposed by generic configuration editing.
use super::*;
use crate::console::secrets::redact_route_document;

#[tokio::test]
async fn cursor_account_switch_and_management_summary_agree_after_commit() {
    let fixture = Fixture::new().await;
    let peer = Peer::new(false).await;
    let input = fixture.body(true);
    fixture.create(&peer, input.clone()).await;
    fixture
        .wait(input["attemptId"].as_str().unwrap(), "succeeded")
        .await;

    let config = "/v1/internal/gateway/console/route-config";
    let inventory = "/v1/internal/gateway/account-groups";
    let mut revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    for enabled in [true, false] {
        let response = fixture.call(Method::GET, config, Value::Null, None).await;
        assert_eq!(response.status(), StatusCode::OK);
        let current = support::parse_json(response).await;
        let mut doc = current["routeConfig"]["document"].clone();
        let credential = &mut doc["providers"][0]["credentials"][0];
        let id = credential["id"].clone();
        assert_eq!(credential["enabled"], !enabled);
        credential["enabled"] = json!(enabled);
        let change = json!({"expectedRevision":revision,"document":doc,"secretPatches":[
            {"path":"/providers/0/credentials/0/auth_token","operation":"keep"},
            {"path":"/providers/1/api_key","operation":"keep"}]});
        let response = fixture
            .call(Method::PUT, config, change, Some(&fixture.grant))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let saved = support::parse_json(response).await;
        assert_eq!(saved["committed"], true);
        let next = saved["routeConfig"]["revision"]["id"].as_str().unwrap();
        assert_ne!(next, revision);
        revision = next.to_owned();

        let response = fixture
            .call(Method::GET, inventory, Value::Null, None)
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let summary = support::parse_json(response).await;
        assert_eq!(summary["summary"]["routeConfigRevision"], revision);
        let accounts = summary["summary"]["accounts"].as_array().unwrap();
        let row = accounts
            .iter()
            .find(|a| a["providerId"] == "cursor" && a["id"] == id)
            .unwrap();
        assert_eq!(row["enabled"], enabled);
        assert!(!summary.to_string().contains(&access()));
        let providers = fixture.state.route_config.snapshot().get_providers();
        let compiled = &providers[0].credential_pool[0];
        assert!(!compiled.enabled && compiled.payload.auth_token.is_none());
    }
    fixture.close().await;
}

#[tokio::test]
async fn cursor_generic_validate_and_commit_reject_type_change_with_kept_token() {
    let fixture = Fixture::new().await;
    let peer = Peer::new(false).await;
    let input = fixture.body(true);
    fixture.create(&peer, input.clone()).await;
    fixture
        .wait(input["attemptId"].as_str().unwrap(), "succeeded")
        .await;
    let active = fixture.state.route_config.snapshot();
    let mut candidate = redact_route_document(active.document()).unwrap().document;
    candidate.providers[0].auth_mode = None;
    candidate.providers[0].base_url = "https://example.invalid/v1".into();
    candidate.providers[0].credentials[0].enabled = Some(true);
    let body = json!({
        "expectedRevision":active.revision().id(), "document":candidate,
        "secretPatches":[{"path":"/providers/0/credentials/0/auth_token","operation":"keep"},
            {"path":"/providers/1/api_key","operation":"keep"}]
    });
    for (method, path) in [
        (
            Method::POST,
            "/v1/internal/gateway/console/route-config/validate",
        ),
        (Method::PUT, "/v1/internal/gateway/console/route-config"),
    ] {
        let response = fixture
            .call(method, path, body.clone(), Some(&fixture.grant))
            .await;
        let value = support::parse_json(response).await;
        assert_eq!(value["error"]["code"], "cursor_import_required", "{value}");
        assert!(!value.to_string().contains(&access()));
        assert_eq!(
            fixture.state.route_config.snapshot().revision().id(),
            active.revision().id()
        );
    }
    // Removal is allowed; changing type may not implicitly carry the old protected secret.
    let mut cleared = candidate;
    cleared.providers[1].api_key = active.document().providers[1].api_key.clone();
    assert!(validate_generic_update(active.document(), &cleared).is_ok());
    let mut copied = cleared;
    copied.providers[1].api_key = access();
    assert_eq!(
        validate_generic_update(active.document(), &copied)
            .unwrap_err()
            .code
            .as_deref(),
        Some("cursor_import_required")
    );
    fixture.close().await;
}
