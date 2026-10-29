//! The existing authenticated UI commit path resets provider health on real dispatch toggles only.
#[path = "console_contract_support/mod.rs"]
mod support;
use axum::http::{Method, StatusCode};
use neuro_gateway::{
    console::secrets::{redact_route_document, SecretPatch},
    db,
    local_runtime::LocalRuntime,
};
use serde_json::json;
use std::sync::Arc;
use support::*;

fn failure(provider: &str, credential: &str, model: &str) -> db::RecordCredentialModelFailureInput {
    db::RecordCredentialModelFailureInput {
        provider_account_id: provider.into(),
        provider_credential_id: None,
        provider_credential_ref: Some(credential.into()),
        protocol_profile: Some("openai".into()),
        model: model.into(),
        upstream_status: Some(401),
        upstream_retry_after_ms: None,
        error_message: Some("invalid API key fixture".into()),
        classification: neuro_gateway::provider_failure::classify_provider_failure(
            Some(401),
            None,
            Some("invalid API key fixture"),
        ),
    }
}

#[tokio::test]
async fn disabling_then_enabling_resets_every_model_but_preserves_other_provider_and_secrets() {
    let original: neuro_gateway::routing::config::RouteConfigYaml=serde_json::from_value(json!({"providers":[
        {"id":"site","preset":"openai","base_url":"http://127.0.0.1:1/v1","credentials":[{"id":"a","api_key":"fixture-a"},{"id":"b","api_key":"fixture-b"}],"supported_models":["m"]},
        {"id":"other","preset":"openai","base_url":"http://127.0.0.1:1/v1","credentials":[{"id":"other-key","api_key":"fixture-other"}],"supported_models":["m"]}
    ],"model_routes":[{"pattern":"m","provider_ids":["site","other"]}],"aliases":{}})).unwrap();
    let mut fixture = ConsoleStateFixture::new(original.clone(), true);
    let root = std::env::temp_dir().join(format!(
        "gateway-local-toggle-health-{}",
        uuid::Uuid::new_v4()
    ));
    let local = LocalRuntime::open(&root).await.unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime = Some(local.clone());
    for (provider, credential, model) in [
        ("site", "a", "m"),
        ("site", "b", "other-model"),
        ("other", "other-key", "m"),
    ] {
        local
            .model_failure(failure(provider, credential, model))
            .await
            .unwrap();
    }
    let grant = grant_console_secret_access(&fixture.state).await;
    // A normal label edit is not an implicit reset.
    let current = fixture.state.route_config.snapshot();
    let redacted = redact_route_document(&original).unwrap();
    let patches: Vec<_> = redacted
        .secrets
        .into_iter()
        .filter(|secret| secret.configured)
        .map(|secret| SecretPatch::keep(secret.path))
        .collect();
    let mut draft = redacted.document;
    draft.providers[0].label = Some("renamed".into());
    let revision = current.revision().id().to_string();
    let response = send_console_json(
        &fixture.state,
        Method::PUT,
        "/v1/internal/gateway/console/route-config",
        json!({"expectedRevision":revision,"document":draft.clone(),"secretPatches":patches}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        parse_json(response).await
    );
    assert!(local
        .list_model_states(Default::default())
        .await
        .unwrap()
        .iter()
        .all(|row| row.status == "blocked"));
    for enabled in [false, true] {
        if enabled {
            // A residual persisted failure must also be cleared on resume, not only on pause.
            local
                .model_failure(failure("site", "a", "m"))
                .await
                .unwrap();
        }
        let current = fixture.state.route_config.snapshot();
        for credential in &mut draft.providers[0].credentials {
            credential.enabled = Some(enabled);
        }
        let response = send_console_json(
            &fixture.state,
            Method::PUT,
            "/v1/internal/gateway/console/route-config",
            json!({"expectedRevision":current.revision().id(),"document":draft.clone(),"secretPatches":patches}),
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{}",
            parse_json(response).await
        );
        let rows = local.list_model_states(Default::default()).await.unwrap();
        for row in rows {
            if row.provider_account_id == "site" {
                assert_eq!(row.status, "active");
                assert_eq!(row.failure_count, 0);
                assert!(row.cooldown_until.is_none());
            } else {
                assert_eq!(row.status, "blocked");
                assert_eq!(row.failure_count, 1);
            }
        }
    }
    let current = fixture.state.route_config.snapshot();
    assert_eq!(current.resolve_candidates(Some("m")).len(), 2);
    // Inspect the concrete candidate rather than exposing secrets through the management response.
    assert!(["fixture-a", "fixture-b"].contains(
        &current.resolve_candidates(Some("m"))[0]
            .payload
            .api_key
            .as_str()
    ));
    local.close().await;
    tokio::fs::remove_dir_all(&root).await.unwrap();
}
