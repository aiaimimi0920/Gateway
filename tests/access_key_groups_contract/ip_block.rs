//! Real HTTP/SSE fallback stays inside the key's group and cools all accounts of the blocked provider.
use super::*;
use axum::response::{IntoResponse, Response};

const BLOCK: &str = "该ip已被封禁，原因：bulk probe guard: ip 127.0.0.1 requested 5 distinct models in 60s (last_user 11552, offense 1)";

async fn respond(
    State(calls): State<Arc<Mutex<Vec<String>>>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let auth = headers
        .get("authorization")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    calls.lock().unwrap().push(auth.clone());
    if auth == "Bearer fixture-blocked" {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":{"message":BLOCK}})),
        )
            .into_response();
    }
    if body["stream"] == true {
        return ([("content-type", "text/event-stream")], concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"fixture success\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n"
        )).into_response();
    }
    Json(json!({"id":"fixture","object":"chat.completion","model":body["model"],
        "choices":[{"index":0,"message":{"role":"assistant","content":"fixture success"},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}})).into_response()
}

async fn exercise(stream: bool) {
    let root = std::env::temp_dir().join(format!("gateway-ip-block-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/v1/chat/completions", post(respond))
        .with_state(calls.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut config = support::test_config();
    config.storage_mode = GatewayStorageMode::Local;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    let mut document = json!({"providers":[
        {"id":"blocked","preset":"openai","base_url":base,"supported_models":["shared","other"],
         "credentials":[{"id":"a1","api_key":"fixture-blocked","supported_models":["shared","other"]},
             {"id":"a2","api_key":"fixture-sibling","supported_models":["shared","other"]}]},
        {"id":"backup","preset":"openai","base_url":base,"supported_models":["shared","other"],
         "credentials":[{"id":"b","api_key":"fixture-backup","supported_models":["shared","other"]}]},
        {"id":"outside","preset":"openai","base_url":base,"supported_models":["shared","other"],
         "credentials":[{"id":"c","api_key":"fixture-outside","supported_models":["shared","other"]}]}],
        "account_groups":[{"id":"both","name":"Both","provider_credential_ids":["a1","a2","b"]},
            {"id":"only-blocked","name":"Only Blocked","provider_credential_ids":["a1","a2"]}],
        "model_routes":[{"pattern":"*","provider_ids":["blocked"],"priority":100}]});
    // This contract owns immediate IP-block fallback; recovery waiting has its own tests.
    for provider in document["providers"].as_array_mut().unwrap() {
        provider["request_wait"] = json!({"mode":"disabled"});
    }
    std::fs::write(&config.console.routes_file, document.to_string()).unwrap();
    let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
        .await
        .unwrap();
    let (status, key) = api(
        &state,
        ROOT,
        support::MANAGEMENT_TOKEN,
        Some(input(json!(["both"]))),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{key}");
    let token = key["token"].as_str().unwrap();
    let (status, reply) = api(&state, "/v1/chat/completions", token,
        Some(json!({"model":"shared","messages":[{"role":"user","content":"fixture"}],"stream":stream,
            "tools":[{"type":"function","function":{"name":"fixture","parameters":{"type":"object"}}}]}))).await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    assert!(reply.to_string().contains("fixture success"), "{reply}");
    assert_eq!(
        *calls.lock().unwrap(),
        vec!["Bearer fixture-blocked", "Bearer fixture-backup"]
    );
    assert_eq!(relay(&state, token, "other").await, StatusCode::OK);
    assert_eq!(
        calls.lock().unwrap().last().unwrap(),
        "Bearer fixture-backup"
    );
    assert_eq!(
        calls.lock().unwrap().len(),
        3,
        "provider ban covers its other models and sibling accounts"
    );

    let (status, limited) = api(
        &state,
        ROOT,
        support::MANAGEMENT_TOKEN,
        Some(input(json!(["only-blocked"]))),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{limited}");
    let (status, reply) = api(
        &state,
        "/v1/chat/completions",
        limited["token"].as_str().unwrap(),
        Some(json!({"model":"shared","messages":[{"role":"user","content":"fixture"}]})),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{reply}");
    assert!(reply
        .to_string()
        .contains("authorized_candidates_unavailable"));
    assert_eq!(
        relay(&state, token, "unentitled-model").await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        calls.lock().unwrap().len(),
        3,
        "no unauthorized provider or sibling sends"
    );
    state.local_runtime.as_ref().unwrap().close().await;
    drop(state);
    task.abort();
    let _ = task.await;
    // Retain synthetic fixture state under the caller's isolated TEMP for diagnosis.
}

#[tokio::test]
async fn ip_block_http_switches_authorized_provider_without_poisoning_credentials() {
    Box::pin(exercise(false)).await;
}

#[tokio::test]
async fn ip_block_stream_start_switches_authorized_provider_without_replaying_tools() {
    Box::pin(exercise(true)).await;
}
