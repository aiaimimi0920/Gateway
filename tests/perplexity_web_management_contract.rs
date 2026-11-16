//! Opt-in real management HTTP save of the ordinary frontend apply/SecretPatch export, no credentials.
use axum::http::{Method, StatusCode};
use neuro_gateway::{
    console::secrets::SecretPatch, routing::config::RouteConfigYaml,
    upstream::perplexity_web::Session,
};
use serde::Deserialize;
use serde_json::{json, Value};
#[path = "console_contract_support/mod.rs"]
mod support;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};
const ENDPOINT: &str = "/v1/internal/gateway/console/route-config";
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrontendFixture {
    product: String,
    active: RouteConfigYaml,
    document: RouteConfigYaml,
    secret_patches: Vec<SecretPatch>,
}

#[tokio::test]
#[ignore = "requires the explicitly exported synthetic normal-frontend fixture"]
async fn perplexity_web_management_frontend_save_grant_cas_and_keep_binding() {
    let path =
        std::env::var("GATEWAY_SESSION_MATERIAL_FIXTURE_PATH").expect("synthetic fixture path");
    let fixtures: Vec<FrontendFixture> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let source = fixtures
        .into_iter()
        .find(|fixture| fixture.product == "perplexity-web")
        .unwrap();
    let fixture = ConsoleStateFixture::new(source.active, true);
    let original = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let candidate = json!({"document":source.document,"secretPatches":source.secret_patches,"expectedRevision":original});
    let missing = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        candidate.clone(),
        None,
        None,
    )
    .await;
    assert_eq!(missing.status(), StatusCode::FORBIDDEN);
    let grant = grant_console_secret_access(&fixture.state).await;
    let wrong = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        candidate.clone(),
        Some(&grant),
        Some("https://wrong-origin.invalid"),
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        original
    );
    let saved = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        candidate.clone(),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(saved.status(), StatusCode::OK);
    let saved = parse_json(saved).await;
    let public = saved.to_string();
    for secret in ["synthetic-new-token", "synthetic-old-session"] {
        assert!(!public.contains(secret));
    }
    let snapshot = fixture.state.route_config.snapshot();
    let revision = snapshot.revision().id().to_owned();
    assert_ne!(revision, original);
    let providers = fixture.state.route_config.get_providers();
    let account = &providers
        .iter()
        .find(|provider| provider.id == "renamed")
        .unwrap()
        .credential_pool[0];
    assert_eq!(account.id, "account");
    assert_eq!(account.payload.api_key, "synthetic-new-token");
    assert_eq!(account.payload.headers["User-Agent"], "synthetic-agent");
    assert!(Session::from_payload(&account.payload).is_ok());
    let stale = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        candidate,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        revision
    );

    let keeps: Vec<_> = saved["routeConfig"]["secrets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|secret| secret["configured"] == true)
        .map(|secret| SecretPatch::keep(secret["path"].as_str().unwrap()))
        .collect();
    let mut changed = saved["routeConfig"]["document"].clone();
    let provider = changed["providers"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|provider| provider["id"] == "renamed")
        .unwrap();
    provider["credentials"][0]["headers"]["User-Agent"] = json!("other-UA");
    let changed = json!({"document":changed,"secretPatches":keeps,"expectedRevision":revision});
    let refused = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        changed,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    let refused: Value = parse_json(refused).await;
    assert_eq!(refused["error"]["code"], "secret_keep_destination_changed");
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        revision
    );
}
