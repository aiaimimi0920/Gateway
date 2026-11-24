//! An aggregator's directory is not evidence that every listed channel works.
use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use neuro_gateway::provider_discovery::{discover, DiscoveredProtocol};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[tokio::test]
#[ignore = "Explicit operator-authorized live discovery; may incur upstream charges"]
async fn live_authorized_discovery() {
    let base = std::env::var("GATEWAY_DISCOVERY_BASE").expect("explicit base required");
    let key = std::env::var("GATEWAY_DISCOVERY_KEY").expect("explicit key required");
    let path = std::env::var("GATEWAY_DISCOVERY_OUTPUT").expect("evidence path required");
    let result = discover(&base, &key).await.expect("live discovery failed");
    let evidence = serde_json::to_string_pretty(&result).unwrap();
    assert!(!evidence.contains(&key));
    std::fs::write(path, evidence).unwrap();
    println!(
        "models={} confirmed_protocols={}",
        result.models.len(),
        result.protocols.len()
    );
    for capability in result.protocols {
        println!(
            "{:?}: {:?}",
            capability.protocol, capability.verified_models
        );
    }
}

#[tokio::test]
async fn discovery_skips_broken_variants_and_model_scoped_auth_failures() {
    for rejection in [
        StatusCode::INTERNAL_SERVER_ERROR,
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
    ] {
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured = seen.clone();
        let app = Router::new()
            .route(
                "/v1/models",
                get(|| async {
                    Json(json!({"data":[
                        {"id":"deepseek-flash-free"},
                        {"id":"deepseek-v4-flash"},
                        {"id":"deepseek-v4-flash-free"},
                        {"id":"fixture-chat"}
                    ]}))
                }),
            )
            .route(
                "/v1/chat/completions",
                post(move |Json(body): Json<Value>| {
                    let captured = captured.clone();
                    async move {
                        let model = body["model"].as_str().unwrap().to_owned();
                        captured.lock().unwrap().push(model.clone());
                        if model == "fixture-chat" {
                            (
                                StatusCode::OK,
                                Json(json!({"choices":[{"message":{"content":"OK"}}]})),
                            )
                        } else {
                            (
                                rejection,
                                Json(json!({"error":{"message":"Model channel unavailable"}})),
                            )
                        }
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let root = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let result = discover(&root, "synthetic-key").await;
        server.abort();
        let _ = server.await;
        let discovery = result.unwrap();
        assert_eq!(discovery.protocol, DiscoveredProtocol::ChatCompletions);
        assert_eq!(discovery.verified_models, ["fixture-chat"]);
        assert_eq!(
            *seen.lock().unwrap(),
            ["deepseek-flash-free", "fixture-chat"]
        );
        assert!(discovery
            .probes
            .iter()
            .all(|probe| probe.attempts.len() <= 24));
    }
}
