//! Multi-wire discovery is persisted; native inference preserves provider extensions.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use neuro_gateway::{
    http::router::build_router,
    provider_discovery::discover,
    routing::config::{RouteConfigStore, RouteConfigYaml},
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

const CHAT: &str = "/v1/chat/completions";
const RESPONSES: &str = "/v1/responses";
const MESSAGES: &str = "/messages";
const SSE: &str = ": vendor comment\nid: fixture-event\nevent: vendor.extension\ndata: {\"vendor_extra\":true}\n\ndata: [DONE]\n\n";

struct Mock {
    root: String,
    reads: Arc<AtomicUsize>,
    requests: Arc<Mutex<Vec<(String, Value)>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Mock {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Mock {
    async fn start() -> Self {
        let reads = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let count = reads.clone();
        let mut router = Router::new().route(
            "/v1/models",
            get(move || {
                let count = count.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Json(json!({"data":[{"id":"actual"},{"id":"catalog-only"}]}))
                }
            }),
        );
        for path in [CHAT, RESPONSES, MESSAGES] {
            let captured = requests.clone();
            router = router.route(path, post(move |Json(body): Json<Value>| {
                let captured = captured.clone();
                async move {
                    captured.lock().unwrap().push((path.into(), body.clone()));
                    if body["stream"] == true { return ([("content-type", "text/event-stream")], SSE).into_response(); }
                    let mut reply = match path {
                        CHAT => json!({"id":"chat-fixture","object":"chat.completion","created":1,"model":"actual",
                            "choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],
                            "usage":{"prompt_tokens":4,"completion_tokens":1,"total_tokens":5}}),
                        RESPONSES => json!({"id":"resp-fixture","object":"response","status":"completed","model":"actual",
                            "output":[{"type":"message","id":"msg-fixture","role":"assistant","status":"completed","content":[{"type":"output_text","text":"OK","annotations":[]}]}],
                            "usage":{"input_tokens":4,"output_tokens":1,"total_tokens":5}}),
                        _ => json!({"id":"msg-fixture","type":"message","role":"assistant","model":"actual",
                            "content":[{"type":"text","text":"OK"}],"stop_reason":"end_turn","usage":{"input_tokens":4,"output_tokens":1}}),
                    };
                    reply["vendor_extra"] = json!({"keep":[1,2,3]});
                    Json(reply).into_response()
                }
            }));
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let root = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self {
            root,
            reads,
            requests,
            task,
        }
    }
}

#[tokio::test]
async fn discovers_all_wires_and_preserves_native_json_and_sse_after_reload() {
    let mock = Mock::start().await;
    let discovery = discover(&mock.root, "fixture-secret").await.unwrap();
    assert_eq!(
        discovery.protocols.len(),
        3,
        "Chat success must not suppress other probes"
    );
    assert_eq!(
        discovery.protocols[2].api_base, mock.root,
        "Protocols may have different base paths"
    );
    let config: RouteConfigYaml = serde_json::from_value(json!({"providers":[{
        "id":"custom-api-provider","adapter":"openai_compatible","base_url":mock.root,
        "model_map":{"alias":"actual"},"credentials":[{"id":"existing-account","api_key":"fixture-secret","discovery":discovery}]}],"model_routes":[]})).unwrap();
    let config: RouteConfigYaml =
        serde_yaml::from_str(&serde_yaml::to_string(&config).unwrap()).unwrap();
    let fixture = ConsoleStateFixture::new(config, true);
    mock.requests.lock().unwrap().clear();
    for (path, mut body) in [
        (
            CHAT,
            json!({"model":"alias","messages":[{"role":"system","content":"one"},{"role":"system","content":"two"},{"role":"user","content":"hello"}],
            "tools":[{"type":"function","function":{"name":"lookup","parameters":{"type":"object"},"strict":true}}]}),
        ),
        (
            RESPONSES,
            json!({"model":"alias","input":"hello","previous_response_id":"resp-existing","tools":[{"type":"web_search"}]}),
        ),
        (
            "/v1/messages",
            json!({"model":"alias","max_tokens":32,"messages":[{"role":"user","content":"hello"}],"system":[{"type":"text","text":"one","cache_control":{"type":"ephemeral"}}]}),
        ),
    ] {
        body["vendor_request"] = json!({"preserve":true});
        for stream in [false, true] {
            body["stream"] = json!(stream);
            let response = build_router(fixture.state.clone())
                .oneshot(
                    Request::post(path)
                        .header("authorization", "Bearer public-api-contract-secret")
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
                .await
                .unwrap();
            assert_eq!(
                status,
                StatusCode::OK,
                "{path}: {}",
                String::from_utf8_lossy(&bytes)
            );
            if stream {
                assert_eq!(
                    bytes.as_ref(),
                    SSE.as_bytes(),
                    "Native SSE must not be re-encoded"
                );
            } else {
                let reply: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(reply["vendor_extra"], json!({"keep":[1,2,3]}));
                assert!(reply["usage"].is_object());
            }
            let sent = mock.requests.lock().unwrap().last().unwrap().clone();
            assert_eq!(
                sent.0,
                if path == "/v1/messages" {
                    MESSAGES
                } else {
                    path
                }
            );
            let mut expected = body.clone();
            expected["model"] = json!("actual");
            assert_eq!(
                sent.1, expected,
                "Native requests must retain extensions and tools"
            );
        }
    }
    assert_eq!(
        mock.reads.load(Ordering::SeqCst),
        1,
        "Inference must not discover again"
    );
    assert_eq!(mock.requests.lock().unwrap().len(), 6);
}

#[tokio::test]
async fn selection_prefers_native_respects_negative_evidence_and_rejects_lossy_fallback() {
    use neuro_gateway::{
        protocol::{canonical::EndpointKind, responses::normalize_responses},
        provider_discovery::DiscoveredProtocol,
        routing::protocol_resolution::finalize_candidate_protocol_family,
    };
    let mock = Mock::start().await;
    let discovery = discover(&mock.root, "fixture-secret").await.unwrap();
    let config: RouteConfigYaml = serde_json::from_value(json!({"providers":[{
        "id":"pool","adapter":"openai_compatible","base_url":mock.root,
        "credentials":[{"id":"account","api_key":"fixture-secret","discovery":discovery}]}],"model_routes":[]})).unwrap();
    let store = RouteConfigStore::from_document(config).unwrap();
    let mut request = normalize_responses(json!({"model":"catalog-only","input":"hello"})).unwrap();
    let original = store.resolve_candidates(Some("catalog-only")).remove(0);
    let mut candidate = original.clone();
    candidate.payload.discovered_protocols[0]
        .verified_models
        .push("catalog-only".into());
    assert_eq!(
        finalize_candidate_protocol_family(&mut candidate, &request),
        Some(true)
    );
    assert_eq!(
        candidate.protocol_family,
        DiscoveredProtocol::Responses.family()
    );
    let mut candidate = original.clone();
    candidate.payload.discovered_protocols[1]
        .failed_models
        .push("catalog-only".into());
    assert_eq!(
        finalize_candidate_protocol_family(&mut candidate, &request),
        Some(false)
    );
    assert_eq!(
        candidate.protocol_family,
        DiscoveredProtocol::ChatCompletions.family()
    );
    for extra in [
        json!({"previous_response_id":"old"}),
        json!({"tools":[{"type":"web_search"}]}),
        json!({"background":true}),
    ] {
        let mut candidate = original.clone();
        candidate
            .payload
            .discovered_protocols
            .retain(|c| c.protocol != DiscoveredProtocol::Responses);
        let mut body = json!({"model":"catalog-only","input":"hello"});
        body.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let req = normalize_responses(body).unwrap();
        assert_eq!(
            finalize_candidate_protocol_family(&mut candidate, &req),
            None
        );
    }
    let mut candidate = original.clone();
    candidate.supported_protocol_families = vec!["unavailable".into()];
    assert_eq!(
        finalize_candidate_protocol_family(&mut candidate, &request),
        None
    );
    request.endpoint_kind = EndpointKind::Embeddings;
    let mut candidate = original;
    assert_eq!(
        finalize_candidate_protocol_family(&mut candidate, &request),
        None
    );
}
