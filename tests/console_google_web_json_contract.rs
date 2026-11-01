//! Offline synthetic import through real management routes; no listening socket or browser.
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use neuro_gateway::{http::router::build_router, routing::config::RouteConfigYaml};
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};
use tower::ServiceExt;

#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;
const ENDPOINT: &str = "/v1/internal/gateway/console/gemini-auth-sessions";

fn fixture() -> ConsoleStateFixture {
    static OBJECTS: OnceLock<std::path::PathBuf> = OnceLock::new();
    OBJECTS.get_or_init(|| {
        let root =
            std::env::temp_dir().join(format!("google-json-objects-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::env::set_var("AI_GATEWAY_OBJECT_STORAGE_DRIVER", "local");
        std::env::set_var("AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR", &root);
        root
    });
    let doc: RouteConfigYaml = serde_json::from_value(json!({"providers":[
        {"id":"web-pool","preset":"gemini-web-chat","base_url":"https://gemini.google.com","explicit_credentials_only":true,"credentials":[]},
        {"id":"canvas-pool","preset":"gemini-canvas-browser-relay","base_url":"https://gemini.google.com","explicit_credentials_only":true,"credentials":[]},
        {"id":"studio-pool","preset":"aistudio-web-reverse","base_url":"https://aistudio.google.com","explicit_credentials_only":true,"credentials":[]},
        {"id":"flow-pool","preset":"flow-web","base_url":"https://flow.google.com","explicit_credentials_only":true,"credentials":[]},
        {"id":"stitch-pool","preset":"stitch-web","base_url":"https://stitch.withgoogle.com","explicit_credentials_only":true,"credentials":[]}
    ],"model_routes":[],"aliases":{},"account_groups":[]})).unwrap();
    ConsoleStateFixture::new(doc, true)
}

fn source(family: &str) -> String {
    let names = if family == "stitch-web" {
        vec!["OSID", "__Secure-OSID"]
    } else if family == "gemini-web" || family == "flow-web" {
        vec!["__Secure-1PSID"]
    } else {
        vec!["SAPISID", "__Secure-1PAPISID", "__Secure-3PAPISID"]
    };
    json!({"schemaVersion":1,"provider":family,"authUser":"0","cookies":names.into_iter().map(|name|json!({
        "name":name,"value":"synthetic-private-cookie","domain":if family == "stitch-web" {"stitch.withgoogle.com"} else {".google.com"},"path":"/","expires":-1,"secure":true
    })).collect::<Vec<_>>()} ).to_string()
}
fn request(f: &ConsoleStateFixture, family: &str, id: &str) -> Value {
    json!({"targetFamily":family,"providerId":id,"expectedRevision":f.state.route_config.snapshot().revision().id(),"accountLabel":"JSON fixture","webSessionJson":source(family)})
}

async fn active_document(f: &ConsoleStateFixture) -> Value {
    let response = send_console_json(
        &f.state,
        Method::GET,
        "/v1/internal/gateway/console/route-config",
        Value::Null,
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    parse_json(response).await["routeConfig"]["document"].clone()
}

#[tokio::test]
async fn requires_management_and_secret_access_without_creating_a_browser_session() {
    let f = fixture();
    let body = request(&f, "aistudio-web", "studio-pool");
    let response = build_router(Arc::clone(&f.state))
        .oneshot(
            Request::post(ENDPOINT)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    for grant in [None, Some("invalid-grant")] {
        let response =
            send_console_json(&f.state, Method::POST, ENDPOINT, body.clone(), grant, None).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            parse_json(response).await["error"]["code"],
            "console_secret_access_required"
        );
    }
}

#[tokio::test]
async fn rejects_revision_target_format_and_size_without_mutating_active_config() {
    let f = fixture();
    let grant = grant_console_secret_access(&f.state).await;
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let baseline = active_document(&f).await;
    for (field, value, status) in [
        ("expectedRevision", json!("stale"), StatusCode::CONFLICT),
        ("providerId", json!("missing"), StatusCode::NOT_FOUND),
        ("providerId", json!("web-pool"), StatusCode::BAD_REQUEST),
        (
            "webSessionJson",
            json!("not-json-synthetic-private-cookie"),
            StatusCode::BAD_REQUEST,
        ),
        (
            "webSessionJson",
            json!("x".repeat(65537)),
            StatusCode::BAD_REQUEST,
        ),
        ("webSessionJson", Value::Null, StatusCode::BAD_REQUEST),
    ] {
        let mut body = request(&f, "aistudio-web", "studio-pool");
        body[field] = value;
        let response =
            send_console_json(&f.state, Method::POST, ENDPOINT, body, Some(&grant), None).await;
        assert_eq!(response.status(), status);
        assert!(!parse_json(response)
            .await
            .to_string()
            .contains("synthetic-private-cookie"));
    }
    assert_eq!(f.state.route_config.snapshot().revision().id(), revision);
    assert_eq!(active_document(&f).await, baseline);
}

#[tokio::test]
async fn imports_all_five_families_offline_through_no_store_target_bound_drafts() {
    let f = fixture();
    let grant = grant_console_secret_access(&f.state).await;
    let baseline = active_document(&f).await;
    for (family, id) in [
        ("gemini-web", "web-pool"),
        ("gemini-canvas", "canvas-pool"),
        ("aistudio-web", "studio-pool"),
        ("flow-web", "flow-pool"),
        ("stitch-web", "stitch-pool"),
    ] {
        let response = send_console_json(
            &f.state,
            Method::POST,
            ENDPOINT,
            request(&f, family, id),
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let body = parse_json(response).await;
        assert_eq!(body["session"]["status"], "succeeded");
        assert_eq!(body["session"]["providerId"], id);
        assert!(body["session"]["message"]
            .as_str()
            .unwrap()
            .contains("未联网验证"));
        let draft = &body["session"]["generatedDrafts"][0];
        assert_eq!(draft["providerId"], id);
        assert_eq!(draft["credential"]["account_name"], "JSON fixture");
        assert!(!draft["credential"]
            .to_string()
            .contains("synthetic-private-cookie"));
        assert_eq!(draft["credential"]["model_selection"]["fetched"], json!([]));
        if family == "flow-web" {
            assert_eq!(draft["credential"]["supported_models"], json!([]));
        } else if family == "stitch-web" {
            assert_eq!(
                draft["credential"]["supported_models"],
                json!(["stitch-design"])
            );
        }
        if family != "gemini-web" {
            assert!(!body.to_string().contains("synthetic-private-cookie"));
            let key = draft["credential"]["runtime_state_object_key"]
                .as_str()
                .unwrap();
            let bytes = neuro_gateway::object_storage::gateway_object_storage()
                .unwrap()
                .read_bytes(key)
                .await
                .unwrap();
            let stored: Value = serde_json::from_slice(&bytes).unwrap();
            assert!(stored.get("origins").is_none());
            assert_eq!(stored["cookies"][0]["value"], "synthetic-private-cookie");
        } else {
            assert_eq!(draft["secretEdits"].as_array().unwrap().len(), 2);
        }
        let session_id = body["session"]["id"].as_str().unwrap();
        let read_back = send_console_json(
            &f.state,
            Method::GET,
            &format!("{ENDPOINT}/{session_id}"),
            Value::Null,
            None,
            None,
        )
        .await;
        assert_eq!(read_back.status(), StatusCode::NOT_FOUND);
    }
    // A returned import draft does not silently rewrite the route document before normal CAS save.
    assert_eq!(active_document(&f).await, baseline);
}

#[tokio::test]
async fn rejects_non_official_base_and_oversized_envelopes() {
    let doc: RouteConfigYaml = serde_json::from_value(json!({"providers":[{
        "id":"studio-pool","preset":"aistudio-web-reverse","base_url":"https://example.invalid","credentials":[]
    }],"model_routes":[],"aliases":{},"account_groups":[]})).unwrap();
    let f = ConsoleStateFixture::new(doc, true);
    let grant = grant_console_secret_access(&f.state).await;
    let response = send_console_json(
        &f.state,
        Method::POST,
        ENDPOINT,
        request(&f, "aistudio-web", "studio-pool"),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let mut body = request(&f, "aistudio-web", "studio-pool");
    body["webSessionJson"] = json!("x".repeat(512 * 1024));
    let response =
        send_console_json(&f.state, Method::POST, ENDPOINT, body, Some(&grant), None).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
