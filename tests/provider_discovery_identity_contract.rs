//! Probe identity follows each wire format and keeps account authentication unchanged.
use axum::{extract::Request, http::StatusCode, routing::any, Json, Router};
use neuro_gateway::provider_discovery::{discover, DiscoveredProtocol};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn directory_and_openai_probes_use_codex_while_messages_uses_claude() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let captured = calls.clone();
    let upstream = Router::new().fallback(any(move |request: Request| {
        let captured = captured.clone();
        async move {
            let headers = request.headers();
            let header = |name| {
                headers
                    .get(name)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
            };
            let path = request.uri().path();
            let codex = header("user-agent").starts_with("codex_exec/")
                && header("originator") == "codex_exec"
                && header("authorization") == "Bearer selected-key"
                && headers.get_all("authorization").iter().count() == 1
                && headers.get_all("content-type").iter().count() == 1
                && uuid::Uuid::parse_str(header("x-codex-window-id")).is_ok()
                && header("x-api-key").is_empty()
                && header("x-app").is_empty();
            let claude = header("user-agent").starts_with("claude-cli/")
                && header("x-app") == "cli"
                && header("x-stainless-lang") == "js"
                && header("x-stainless-runtime") == "node"
                && header("anthropic-version") == "2023-06-01"
                && header("x-api-key") == "selected-key"
                && headers.get_all("x-api-key").iter().count() == 1
                && headers.get_all("anthropic-version").iter().count() == 1
                && header("authorization").is_empty()
                && header("originator").is_empty()
                && header("x-codex-window-id").is_empty();
            let body = match path {
                "/v1/models" if codex => json!({"data":[{"id":"claude-format-fixture"}]}),
                "/v1/chat/completions" if codex => {
                    json!({"choices":[{"message":{"content":"OK"}}]})
                }
                "/v1/responses" if codex => json!({"output":[{"content":[{"text":"OK"}]}]}),
                "/v1/messages" if claude => json!({"content":[{"type":"text","text":"OK"}]}),
                _ => {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(json!({"error":"wrong identity or endpoint"})),
                    )
                }
            };
            captured
                .lock()
                .unwrap()
                .push((path.to_owned(), header("x-codex-window-id").to_owned()));
            (StatusCode::OK, Json(body))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    let result = discover(&base, "selected-key").await.unwrap();
    assert_eq!(result.models, ["claude-format-fixture"]);
    assert_eq!(
        result
            .protocols
            .iter()
            .map(|p| p.protocol)
            .collect::<Vec<_>>(),
        [
            DiscoveredProtocol::ChatCompletions,
            DiscoveredProtocol::Responses,
            DiscoveredProtocol::Messages,
        ]
    );
    let observed = calls.lock().unwrap().clone();
    for path in [
        "/v1/models",
        "/v1/chat/completions",
        "/v1/responses",
        "/v1/messages",
    ] {
        assert!(observed.iter().any(|(p, _)| p == path), "missing {path}");
    }
    let contexts: Vec<_> = observed
        .iter()
        .map(|(_, id)| id)
        .filter(|id| !id.is_empty())
        .collect();
    assert_eq!(contexts.len(), 3);
    assert_eq!(
        contexts
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        contexts.len(),
        "each probe must own a fresh request context"
    );
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("selected-key"));
    server.abort();
    let _ = server.await;
}
