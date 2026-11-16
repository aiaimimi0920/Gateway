//! A parent default edit pins effective audiences; real management keep/CAS still owns persistence.
use axum::http::{Method, StatusCode};
use neuro_gateway::console::secrets::redact_route_document;
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::{json, Value};

#[path = "console_contract_support/mod.rs"]
mod support;
use support::{parse_json, send_console_json, ConsoleStateFixture};

const BASE: &str = "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";
const OVERRIDE: &str = "https://account-b.cn-hongkong.maas.aliyuncs.com/compatible-mode/v1";
const NEW: &str = "https://workspace-new.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1";

fn active() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"ordinary", "preset":"qwen-official-api",
        "explicit_credentials_only":true, "base_url":BASE, "credentials":[
            {"id":"a", "api_key":"synthetic-inherited", "enabled":false, "supported_models":["fetched","manual"],
                "model_selection":{"fetched":["fetched"],"manual":["manual"]},
                "model_catalog":{"sourceUrl":BASE.replace("/compatible-mode/v1", "/api/v1/models"),
                    "accountBaseUrl":BASE,"checkedAt":"2026-11-02T00:00:00Z","models":["fetched"]}},
            {"id":"b", "api_key":"synthetic-override", "base_url":OVERRIDE}]}],
        "model_routes":[{"pattern":"fetched", "provider_ids":["ordinary"], "enabled":false, "priority":42}], "aliases":{}})).unwrap()
}

fn candidate(fixture: &ConsoleStateFixture) -> Value {
    let redacted = redact_route_document(&active()).unwrap();
    let mut document = serde_json::to_value(redacted.document).unwrap();
    document["providers"][0]["base_url"] = json!(NEW);
    document["providers"][0]["credentials"][0]["base_url"] = json!(BASE);
    let keeps: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|secret| secret.configured)
        .map(|secret| json!({"path":secret.path,"operation":"keep"}))
        .collect();
    json!({"document":document,"secretPatches":keeps,
        "expectedRevision":fixture.state.route_config.snapshot().revision().id()})
}

async fn persists() {
    let fixture = ConsoleStateFixture::new(active(), true);
    let request = candidate(&fixture);
    let endpoint = "/v1/internal/gateway/console/route-config";
    // Deliberately no Secret Grant: no existing secret gains an audience or consumer.
    let success = send_console_json(
        &fixture.state,
        Method::PUT,
        endpoint,
        request.clone(),
        None,
        None,
    )
    .await;
    let status = success.status();
    let public = parse_json(success).await;
    assert_eq!(status, StatusCode::OK, "{public}");
    assert!(!public.to_string().contains("synthetic-"));
    let document = &public["routeConfig"]["document"];
    assert_eq!(document["providers"][0]["base_url"], NEW);
    assert_eq!(document["providers"][0]["credentials"][0]["base_url"], BASE);
    assert_eq!(
        document["providers"][0]["credentials"][1]["base_url"],
        OVERRIDE
    );
    assert_eq!(
        document["providers"][0]["credentials"][0]["model_catalog"]["accountBaseUrl"],
        BASE
    );
    assert_eq!(
        document["providers"][0]["credentials"][0]["model_selection"]["fetched"],
        json!(["fetched"])
    );
    assert_eq!(document["model_routes"][0]["enabled"], false);
    assert_eq!(document["model_routes"][0]["priority"], 42);
    let providers = fixture.state.route_config.get_providers();
    assert_eq!(
        providers[0].credential_pool[0].payload.api_key,
        "synthetic-inherited"
    );
    assert_eq!(providers[0].credential_pool[0].payload.base_url, BASE);
    assert_eq!(
        providers[0].credential_pool[1].payload.api_key,
        "synthetic-override"
    );
    assert_eq!(providers[0].credential_pool[1].payload.base_url, OVERRIDE);
    assert!(!providers[0].credential_pool[0].enabled);
    let stale = send_console_json(&fixture.state, Method::PUT, endpoint, request, None, None).await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
}

#[test]
fn qwen_parent_default_edit_preserves_keys_catalog_overrides_and_cas_without_secret_grant() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(persists());
        })
        .unwrap()
        .join()
        .unwrap();
}
