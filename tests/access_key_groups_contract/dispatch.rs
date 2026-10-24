//! Authorized fallback must survive a manually disabled preferred provider without widening access.
use super::*;

#[tokio::test]
async fn disabled_preferred_provider_uses_authorized_backup_and_unavailable_is_not_forbidden() {
    let root = std::env::temp_dir().join(format!("gateway-key-dispatch-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/v1/chat/completions", post(upstream))
        .with_state(calls.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut config = support::test_config();
    config.storage_mode = GatewayStorageMode::Local;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    let document = json!({
        "providers":[
            {"id":"preferred","preset":"openai","base_url":format!("http://{address}"),
             "supported_models":["shared"],"credentials":[
                {"id":"a1","supported_models":["shared"],"api_key":"fixture-a1","enabled":false},
                {"id":"a2","supported_models":["shared"],"api_key":"fixture-a2","enabled":false}]},
            {"id":"backup","preset":"openai","base_url":format!("http://{address}"),
             "supported_models":["shared"],"credentials":[{"id":"b","supported_models":["shared"],"api_key":"fixture-b"}]},
            {"id":"outside","preset":"openai","base_url":format!("http://{address}"),
             "supported_models":["shared"],"credentials":[{"id":"c","supported_models":["shared"],"api_key":"fixture-c"}]}],
        "account_groups":[{"id":"both","name":"Both","provider_credential_ids":["a1","a2","b"]},
            {"id":"only-a","name":"Only A","provider_credential_ids":["a1","a2"]}],
        "model_routes":[{"pattern":"shared","provider_ids":["preferred"],"priority":100}]
    });
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
    assert!(models(&state, token).await.contains(&"shared".to_string()));
    assert_eq!(relay(&state, token, "shared").await, StatusCode::OK);
    assert_eq!(*calls.lock().unwrap(), vec!["Bearer fixture-b"]);

    let (status, limited) = api(
        &state,
        ROOT,
        support::MANAGEMENT_TOKEN,
        Some(input(json!(["only-a"]))),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{limited}");
    let (status, body) = api(
        &state,
        "/v1/chat/completions",
        limited["token"].as_str().unwrap(),
        Some(json!({"model":"shared","messages":[{"role":"user","content":"fixture"}]})),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert!(
        body.to_string()
            .contains("authorized_candidates_unavailable"),
        "{body}"
    );
    assert_eq!(
        relay(&state, token, "unentitled-model").await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        calls.lock().unwrap().len(),
        1,
        "no unauthorized fallback or disabled sends"
    );

    let mut disabled: RouteConfigYaml = serde_json::from_value(document).unwrap();
    disabled.model_routes[0].enabled = false;
    state.route_config.replace_document(disabled).unwrap();
    assert!(!models(&state, token).await.contains(&"shared".to_string()));
    assert_ne!(relay(&state, token, "shared").await, StatusCode::OK);
    assert_eq!(
        calls.lock().unwrap().len(),
        1,
        "disabled model rules stay disabled"
    );
    state.local_runtime.as_ref().unwrap().close().await;
    drop(state);
    task.abort();
    let _ = task.await;
    // Keep fixture data under the caller's isolated TEMP for inspection; no production cleanup.
}
