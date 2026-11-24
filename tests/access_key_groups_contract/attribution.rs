//! Loopback proof: exhausted accounts retain rejection facts; account catalogues supply safe backups.
use super::*;

#[tokio::test]
async fn unavailable_accounts_are_attributed_and_concrete_account_model_enables_backup() {
    let root = std::env::temp_dir().join(format!("gateway-routing-audit-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn({
        let calls = calls.clone();
        async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/v1/chat/completions", post(fixture_upstream))
                    .with_state(calls),
            )
            .await
            .unwrap();
        }
    });
    let mut config = support::test_config();
    config.storage_mode = GatewayStorageMode::Local;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    let document = json!({
        "providers":[
            {"id":"preferred","label":"Preferred","preset":"openai","base_url":format!("http://{address}"),
             "supported_models":["shared"],"credentials":[{"id":"a","api_key":"never-log-secret-a","enabled":false,"supported_models":["shared"]}]},
            {"id":"backup","label":"Backup","preset":"openai","base_url":format!("http://{address}"),
             "credentials":[{"id":"b","api_key":"fixture-b","supported_models":["shared"]}]},
            {"id":"outside","preset":"openai","base_url":format!("http://{address}"),
             "supported_models":["shared"],"credentials":[{"id":"c","api_key":"never-log-secret-c","supported_models":["shared"]}]},
            {"id":"other-model","preset":"openai","base_url":format!("http://{address}"),
             "credentials":[{"id":"d","api_key":"never-log-secret-d","supported_models":["different"]}]}],
        "account_groups":[{"id":"both","name":"Both","provider_credential_ids":["a","b","d"]},
            {"id":"only-a","name":"Only A","provider_credential_ids":["a"]}],
        "model_routes":[{"pattern":"shared","provider_ids":["preferred"],"priority":100}]
    });
    std::fs::write(&config.console.routes_file, document.to_string()).unwrap();
    let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
        .await
        .unwrap();
    let (_, limited) = api(
        &state,
        ROOT,
        support::MANAGEMENT_TOKEN,
        Some(input(json!(["only-a"]))),
    )
    .await;
    let (status, body) = api(
        &state,
        "/v1/chat/completions",
        limited["token"].as_str().unwrap(),
        Some(json!({"model":"shared","messages":[{"role":"user","content":"fixture"}]})),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    let audits = state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_audits(&Default::default())
        .await
        .unwrap();
    assert_eq!(audits.len(), 1);
    assert!(audits[0].provider_account_id.is_none());
    assert!(audits[0].upstream_status.is_none());
    let trace = audits[0].route_trace.as_ref().unwrap();
    assert_eq!(trace["routeAttemptCount"], 0);
    assert_eq!(trace["rejectedCandidates"].as_array().unwrap().len(), 1);
    assert_eq!(trace["rejectedCandidates"][0]["providerLabel"], "Preferred");
    assert_eq!(trace["rejectedCandidates"][0]["realCredentialRef"], "a");
    assert_eq!(trace["rejectedCandidates"][0]["reason"], "account_disabled");
    assert!(trace["routeRevision"].is_string());
    assert!(
        trace["failureClass"].is_null(),
        "routing refusal is not an upstream failure"
    );
    assert!(!trace.to_string().contains("never-log-secret"));
    assert!(calls.lock().unwrap().is_empty());

    let (_, entitled) = api(
        &state,
        ROOT,
        support::MANAGEMENT_TOKEN,
        Some(input(json!(["both"]))),
    )
    .await;
    assert_eq!(
        relay(&state, entitled["token"].as_str().unwrap(), "shared").await,
        StatusCode::OK
    );
    assert_eq!(*calls.lock().unwrap(), vec!["Bearer fixture-b"]);
    let audits = state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_audits(&Default::default())
        .await
        .unwrap();
    let completed = audits.iter().find(|row| row.status == "completed").unwrap();
    assert_eq!(completed.provider_account_id.as_deref(), Some("backup"));
    assert_eq!(
        completed.route_trace.as_ref().unwrap()["realCredentialRef"],
        "b"
    );
    assert_eq!(
        completed.route_trace.as_ref().unwrap()["routeAttemptCount"],
        1
    );
    let (status, _) = api(
        &state,
        "/v1/chat/completions",
        entitled["token"].as_str().unwrap(),
        Some(json!({"model":"shared","messages":[{"role":"user","content":"reject"}]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let audits = state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_audits(&Default::default())
        .await
        .unwrap();
    let failed = audits
        .iter()
        .find(|row| row.upstream_status == Some(400))
        .unwrap();
    assert_eq!(failed.provider_account_id.as_deref(), Some("backup"));
    let trace = failed.route_trace.as_ref().unwrap();
    assert_eq!(trace["selectedCandidate"]["providerLabel"], "Backup");
    assert_eq!(trace["selectedCandidate"]["realCredentialRef"], "b");
    assert_eq!(trace["routeAttemptCount"], 1);
    assert!(!trace.to_string().contains("fixture-b"));
    // Whole-rule disable still forbids a backup; neither other models nor outside accounts are sent.
    let mut disabled: RouteConfigYaml = serde_json::from_value(document).unwrap();
    disabled.model_routes[0].enabled = false;
    state.route_config.replace_document(disabled).unwrap();
    assert_ne!(
        relay(&state, entitled["token"].as_str().unwrap(), "shared").await,
        StatusCode::OK
    );
    assert_eq!(calls.lock().unwrap().len(), 2);
    state.local_runtime.as_ref().unwrap().close().await;
    task.abort();
    let _ = task.await;
}

async fn fixture_upstream(
    State(calls): State<Arc<Mutex<Vec<String>>>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let rejects = body["messages"][0]["content"] == "reject";
    let response = upstream(State(calls), headers, Json(body)).await;
    if rejects {
        (
            StatusCode::BAD_REQUEST,
            Json(
                json!({"error":{"message":"synthetic invalid request","code":"invalid_request_error","type":"invalid_request_error"}}),
            ),
        )
    } else {
        (StatusCode::OK, response)
    }
}
