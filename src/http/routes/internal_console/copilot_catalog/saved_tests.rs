//! Use the existing management commit transaction, not an alternate SDK credential database.
use super::*;
use crate::console::secrets::{redact_route_document, SecretPatch};
use axum::http::Method;

fn saved_fixture() -> ConsoleStateFixture {
    ConsoleStateFixture::new(
        serde_json::from_value(json!({"providers":[{
            "id":"copilot", "auth_mode":"copilot_sdk", "base_url":"",
            "explicit_credentials_only":true, "credentials":[{"id":"saved", "enabled":false}]
        }]}))
        .unwrap(),
        true,
    )
}

fn save_body(f: &ConsoleStateFixture, token: Option<&str>) -> Value {
    let snapshot = f.state.route_config.snapshot();
    let redacted = redact_route_document(snapshot.document()).unwrap();
    let path = "/providers/0/credentials/0/copilot_github_token";
    let mut patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| s.path != path)
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    patches.push(match token {
        Some(token) => SecretPatch::replace(path, json!(token)),
        None => SecretPatch::clear(path),
    });
    json!({"expectedRevision": snapshot.revision().id(), "document":redacted.document,
        "secretPatches":patches})
}

async fn save(f: &ConsoleStateFixture, body: Value, grant: Option<&str>) -> Response {
    support::send_console_json(
        &f.state,
        Method::PUT,
        "/v1/internal/gateway/console/route-config",
        body,
        grant,
        None,
    )
    .await
}

async fn finish(f: &ConsoleStateFixture) {
    f.state
        .shutdown
        .request("synthetic credential test completed");
    tokio::time::timeout(
        Duration::from_secs(45),
        f.state.lifecycle.background_tasks.join(),
    )
    .await
    .unwrap()
    .unwrap();
}

#[tokio::test]
async fn copilot_saved_identity_uses_grant_cas_and_secret_patches() {
    let f = saved_fixture();
    let grant = grant_console_secret_access(&f.state).await;
    let input = save_body(&f, Some("catalog-a"));
    let response = save(&f, input.clone(), None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(
        f.state.route_config.snapshot().document().providers[0].credentials[0]
            .copilot_github_token
            .is_none()
    );
    let response = save(&f, input.clone(), Some(&grant)).await;
    let status = response.status();
    let value = parse_json(response).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert!(!value.to_string().contains("catalog-a"));
    assert_eq!(
        f.state.route_config.snapshot().document().providers[0].credentials[0]
            .copilot_github_token
            .as_deref(),
        Some("catalog-a")
    );
    assert_eq!(
        save(&f, input, Some(&grant)).await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        save(&f, save_body(&f, None), Some(&grant)).await.status(),
        StatusCode::OK
    );
    let snapshot = f.state.route_config.snapshot();
    let credential = &snapshot.document().providers[0].credentials[0];
    assert_eq!(credential.id.as_deref(), Some("saved"));
    assert!(credential.copilot_github_token.is_none());
    finish(&f).await;
}

#[tokio::test]
#[ignore = "requires synthetic-runtime and isolated GATEWAY_PROBE_ROOT"]
async fn copilot_saved_catalog_rejects_old_revision_after_token_rotation() {
    let f = saved_fixture();
    let grant = grant_console_secret_access(&f.state).await;
    let app = mount_with_settings(Router::new(), Some(settings())).with_state(f.state.clone());
    let mut old = None;
    for (token, model) in [("catalog-a", "model-a"), ("catalog-b", "model-b")] {
        let response = save(&f, save_body(&f, Some(token)), Some(&grant)).await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{}",
            parse_json(response).await
        );
        if let Some(old) = old.take() {
            assert_eq!(
                send(app.clone(), old, Some(&grant), true).await.status(),
                StatusCode::CONFLICT
            );
        }
        let input = json!({"providerId":"copilot", "credentialId":"saved",
            "expectedRevision":f.state.route_config.snapshot().revision().id()});
        let response = send(app.clone(), input.clone(), Some(&grant), true).await;
        let status = response.status();
        let value = parse_json(response).await;
        assert_eq!(status, StatusCode::OK, "{value}");
        assert_eq!(value["catalog"]["models"][0]["id"], model);
        assert!(!value.to_string().contains(token));
        old = Some(input);
    }
    let mut ambiguous = old.unwrap();
    ambiguous["githubToken"] = json!("catalog-a");
    assert_eq!(
        send(app, ambiguous, Some(&grant), true).await.status(),
        StatusCode::BAD_REQUEST
    );
    finish(&f).await;
}
