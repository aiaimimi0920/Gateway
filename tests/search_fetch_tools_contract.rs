//! Loopback-only public API and management probes; no paid provider traffic.
use axum::{
    body::Body,
    extract::{OriginalUri, Path, State},
    http::{HeaderMap, Request, StatusCode},
    routing::post,
    Json, Router,
};
use neuro_gateway::{http::router::build_router, state::AppState};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

type Calls = Arc<Mutex<Vec<(String, Value)>>>;

async fn upstream(
    State(calls): State<Calls>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let path = uri.path().trim_start_matches('/').to_string();
    assert!(body.get("model").is_none());
    let url = "https://example.com/article";
    let response = match path.as_str() {
        "v1/contents" => {
            assert_eq!(headers["x-api-key"], "synthetic");
            assert!(headers.get("authorization").is_none());
            assert_eq!(
                body,
                json!({"urls":[url], "formats":["markdown", "metadata"]})
            );
            json!([{"url":url,"title":"Example","markdown":"# Example"}])
        }
        "extract" => {
            assert_eq!(headers["authorization"], "Bearer synthetic");
            assert_eq!(body, json!({"urls":[url]}));
            json!({"results":[{"url":url,"raw_content":"Example"}],"failed_results":[]})
        }
        "scrape" => {
            assert_eq!(headers["authorization"], "Bearer synthetic");
            assert_eq!(body, json!({"url":url,"returnFormat":"markdown"}));
            json!({"code":200,"data":{"url":url,"content":"Example"}})
        }
        "ai-search" => {
            assert_eq!(body, json!({"query":"Rust documentation"}));
            json!({"organic":[{"url":url,"title":"Example"}]})
        }
        _ => panic!("unexpected path {path}"),
    };
    calls.lock().unwrap().push((path, body));
    Json(response)
}

async fn send(state: &Arc<AppState>, endpoint: &str, body: Value) -> (StatusCode, Value) {
    let response = build_router(state.clone())
        .oneshot(
            Request::post(endpoint)
                .header("authorization", "Bearer public-api-contract-secret")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    (response.status(), parse_json(response).await)
}

async fn fail_first_provider(
    State(calls): State<Calls>,
    Path(provider): Path<String>,
) -> (StatusCode, Json<Value>) {
    let mut calls = calls.lock().unwrap();
    let first = calls
        .iter()
        .find(|(path, _)| matches!(path.as_str(), "first" | "second"))
        .map(|(path, _)| path.clone())
        .unwrap_or_else(|| provider.clone());
    calls.push((provider.clone(), Value::Null));
    if first == provider {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"synthetic unavailable"})),
        )
    } else {
        (
            StatusCode::OK,
            Json(json!({"organic":[{"url":"https://example.com", "title":"Example"}]})),
        )
    }
}

#[tokio::test]
async fn search_fetch_tools_public_api_and_exact_credential_probes() {
    let calls = Calls::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/:provider/ai-search", post(fail_first_provider))
        .route("/v1/contents", post(upstream))
        .route("/extract", post(upstream))
        .route("/scrape", post(upstream))
        .route("/ai-search", post(upstream))
        .with_state(calls.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    for (preset, expected_field) in [
        ("you", "markdown"),
        ("tavily", "results"),
        ("websearchapi", "data"),
    ] {
        let fixture = ConsoleStateFixture::new(
            serde_json::from_value(json!({"providers":[{
            "id":preset,"preset":preset,"base_url":format!("http://{address}"),
            "supported_models":[format!("{preset}-search"),format!("{preset}-fetch")],
            "credentials":[{"id":"credential", "api_key":"synthetic"}]
        }],"model_routes":[],"aliases":{}}))
            .unwrap(),
            false,
        );
        for model in [format!("{preset}-fetch"), "fetch".into()] {
            let mut candidates = fixture.state.route_config.resolve_candidates(Some(&model));
            assert!(!candidates.is_empty(), "{preset} {model}: no candidates");
            let request = neuro_gateway::protocol::search_api::normalize_fetch(
                json!({"model":model,"url":"https://example.com/article"}),
            )
            .unwrap();
            assert!(
                neuro_gateway::routing::protocol_resolution::finalize_candidate_protocol_family(
                    &mut candidates[0],
                    &request
                )
                .is_some(),
                "{preset} {model}: incompatible candidate"
            );
            let (status, result) = send(
                &fixture.state,
                "/v1/fetch",
                json!({"model":model,"url":"https://example.com/article"}),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{preset} {model}: {result}");
            assert!(
                if preset == "you" {
                    result[0].get(expected_field).is_some()
                } else {
                    result.get(expected_field).is_some()
                },
                "{result}"
            );
        }
        let grant = grant_console_secret_access(&fixture.state).await;
        let response = build_router(fixture.state.clone()).oneshot(Request::post(format!("/v1/internal/gateway/console/providers/{preset}/probe"))
            .header("x-management-token", MANAGEMENT_TOKEN).header("x-secret-grant", grant)
            .header("content-type", "application/json").body(Body::from(json!({"model":format!("{preset}-fetch"),"prompt":"https://example.com/article","credentialIds":["credential"]}).to_string())).unwrap()).await.unwrap();
        let result = parse_json(response).await;
        assert_eq!(
            result["result"]["results"][0]["status"], "passed",
            "{result}"
        );
        if preset == "websearchapi" {
            let (status, result) = send(
                &fixture.state,
                "/v1/search",
                json!({"model":"search","query":"Rust documentation"}),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{result}");
            assert!(result["organic"].is_array());
            let (status, _) = send(&fixture.state, "/v1/fetch", json!({"model":"websearchapi-fetch","urls":["https://example.com/a","https://example.com/b"]})).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }
    }
    assert_eq!(calls.lock().unwrap().len(), 10);
    let fixture = ConsoleStateFixture::new(serde_json::from_value(json!({"providers":[
        {"id":"first", "preset":"websearchapi", "base_url":format!("http://{address}/first"),"api_key":"synthetic", "supported_models":["websearchapi-search"]},
        {"id":"second", "preset":"websearchapi", "base_url":format!("http://{address}/second"),"api_key":"synthetic", "supported_models":["websearchapi-search"]}
    ],"model_routes":[]})).unwrap(), false);
    // Whichever candidate wins initially remains unavailable; only another
    // authorized provider can succeed, without any explicit model route.
    let (status, body) = send(
        &fixture.state,
        "/v1/search",
        json!({"model":"search", "query":"Rust documentation"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(calls
        .lock()
        .unwrap()
        .iter()
        .any(|(path, _)| path == "first"));
    assert!(calls
        .lock()
        .unwrap()
        .iter()
        .any(|(path, _)| path == "second"));
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn search_fetch_tools_retired_public_balance_never_calls_upstream() {
    let fixture = ConsoleStateFixture::new(document("unrelated", "gpt-test", "synthetic"), false);
    let response = build_router(fixture.state.clone())
        .oneshot(
            Request::get("/v1/credits/balance")
                .header("authorization", "Bearer public-api-contract-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"],
        "unsupported_search_provider_endpoint"
    );
}
