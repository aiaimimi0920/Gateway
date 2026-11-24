//! Public model discovery publishes capability metadata only for admitted exact IDs.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use neuro_gateway::http::router::build_router;
use serde_json::{json, Value};
use tower::ServiceExt;

#[path = "console_contract_support/mod.rs"]
mod support;

async fn get_models(fixture: &support::ConsoleStateFixture, authenticated: bool) -> Response {
    let mut request = Request::get("/v1/models");
    if authenticated {
        let key = fixture.state.config.gateway_api_key.as_deref().unwrap();
        request = request.header("authorization", format!("Bearer {key}"));
    }
    Box::pin(build_router(fixture.state.clone()).oneshot(request.body(Body::empty()).unwrap()))
        .await
        .unwrap()
}

fn fixture(model_routes: Value) -> support::ConsoleStateFixture {
    support::ConsoleStateFixture::new(
        serde_json::from_value(json!({
            "providers": [{
                "id": "fixture", "base_url": "http://127.0.0.1:1", "api_key": "upstream-fixture",
                "supported_models": [
                    "gpt-6.2-astra", "gpt-6.2-sol", "gpt-6.1-sol", "gpt-5.5", "gpt-5.4", "gpt-5.4-mini",
                    "claude-sonnet-4-6", "claude-opus-4-7", "claude-opus-4-8",
                    "gemini-3.1-pro-preview", "gemini-3-flash-preview",
                    "text-embedding-3-small", "gpt-6.2-sol-unverified"
                ]
            }],
            "model_routes": model_routes,
            "aliases": {"coding": "gpt-6.2-sol"}
        }))
        .unwrap(),
        false,
    )
}

#[tokio::test]
async fn catalog_advertises_model_specific_openai_reasoning_levels() {
    let response = get_models(&fixture(json!([])), true).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = support::parse_json(response).await;
    assert_eq!(body["object"], "list");
    let rows = body["data"].as_array().unwrap();
    assert_eq!(rows.len(), 14);
    let gpt6 = ["low", "medium", "high", "xhigh", "max"];
    let gpt5 = ["none", "low", "medium", "high", "xhigh"];
    for (id, default, expected) in [
        ("gpt-6.2-astra", "medium", gpt6),
        ("gpt-6.2-sol", "medium", gpt6),
        ("gpt-6.1-sol", "medium", gpt6),
        ("gpt-5.5", "medium", gpt5),
        ("gpt-5.4", "none", gpt5),
        ("gpt-5.4-mini", "none", gpt5),
    ] {
        let model = rows.iter().find(|row| row["id"] == id).unwrap();
        assert_eq!(model["object"], "model");
        assert!(model["created"].is_i64());
        assert_eq!(model["owned_by"], "neuro-gateway");
        assert_eq!(model["default_reasoning_level"], default, "{id}");
        if matches!(id, "gpt-6.2-sol" | "gpt-6.2-astra") {
            assert_eq!(model["input_modalities"], json!(["text", "image"]), "{id}");
        }
        assert!(model.get("native_reasoning").is_none(), "{id}");
        let levels = model["supported_reasoning_levels"].as_array().unwrap();
        let efforts = levels
            .iter()
            .map(|level| {
                assert!(!level["description"].as_str().unwrap().is_empty());
                level["effort"].as_str().unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(efforts, expected, "{id}");
    }
    for id in ["coding", "gpt-6.2-sol-unverified", "text-embedding-3-small"] {
        let model = rows.iter().find(|row| row["id"] == id).unwrap();
        assert!(model.get("default_reasoning_level").is_none(), "{id}");
        assert!(model.get("supported_reasoning_levels").is_none(), "{id}");
        assert!(model.get("native_reasoning").is_none(), "{id}");
        assert!(model.get("input_modalities").is_none(), "{id}");
    }
}

#[tokio::test]
async fn native_controls_are_not_advertised_as_openai_effort() {
    let response = get_models(&fixture(json!([])), true).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = support::parse_json(response).await;
    let rows = body["data"].as_array().unwrap();
    for (ids, protocol, parameter, levels) in [
        (
            &["claude-sonnet-4-6"][..],
            "anthropic_messages",
            "output_config.effort",
            &["low", "medium", "high", "max"][..],
        ),
        (
            &["claude-opus-4-7", "claude-opus-4-8"][..],
            "anthropic_messages",
            "output_config.effort",
            &["low", "medium", "high", "xhigh", "max"][..],
        ),
        (
            &["gemini-3.1-pro-preview"][..],
            "gemini_generate_content",
            "generationConfig.thinkingConfig.thinkingLevel",
            &["LOW", "MEDIUM", "HIGH"][..],
        ),
        (
            &["gemini-3-flash-preview"][..],
            "gemini_generate_content",
            "generationConfig.thinkingConfig.thinkingLevel",
            &["MINIMAL", "LOW", "MEDIUM", "HIGH"][..],
        ),
    ] {
        for id in ids {
            let model = rows.iter().find(|row| row["id"] == *id).unwrap();
            assert!(model.get("default_reasoning_level").is_none(), "{id}");
            assert!(model.get("supported_reasoning_levels").is_none(), "{id}");
            assert_eq!(
                model["native_reasoning"],
                json!({
                    "protocol": protocol, "parameter": parameter, "supported_levels": levels
                }),
                "{id}"
            );
        }
    }
}

#[tokio::test]
async fn catalog_metadata_does_not_bypass_auth_or_route_admission() {
    let fixture = fixture(json!([
        {"pattern": "gpt-6.2-astra", "provider_ids": ["fixture"], "enabled": false},
        {"pattern": "claude-sonnet-4-6", "provider_ids": ["fixture"], "enabled": false}
    ]));
    let unauthorized = get_models(&fixture, false).await;
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let response = get_models(&fixture, true).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = support::parse_json(response).await;
    let rows = body["data"].as_array().unwrap();
    assert!(rows.iter().any(|row| row["id"] == "gpt-6.2-sol"));
    for id in ["gpt-6.2-astra", "claude-sonnet-4-6"] {
        assert!(rows.iter().all(|row| row["id"] != id), "{id}");
    }
}
