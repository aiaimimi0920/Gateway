//! Opt-in real PostgreSQL/Redis contract; retained fixture data, no production accounts.
use super::*;
use crate::console::{ConsoleConfig, ConsoleConfigValues};

pub(super) async fn call(state: &Arc<AppState>, path: &str, token: &str, body: Value) -> Value {
    let response = build_router(state.clone())
        .oneshot(
            Request::post(path)
                .header("authorization", format!("Bearer {token}"))
                .header("x-management-token", token)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let text = drain(response.into_body()).await;
    assert_eq!(status, StatusCode::OK, "{path}: {text}");
    assert!(!text.contains(KEY));
    serde_json::from_str(&text).unwrap()
}

pub(super) async fn state(
    root: &std::path::Path,
    namespace: &str,
    peer: &peer::Peer,
) -> Arc<AppState> {
    state_document(
        root,
        namespace,
        peer,
        json!({"providers":[],"model_routes":[]}),
    )
    .await
}

pub(super) async fn state_document(
    root: &std::path::Path,
    namespace: &str,
    peer: &peer::Peer,
    document: Value,
) -> Arc<AppState> {
    std::fs::create_dir_all(root).unwrap();
    let console = ConsoleConfig::from_values(ConsoleConfigValues {
        routes_file: Some(root.join("routes.yaml")),
        state_dir: Some(root.join("state")),
        redis_namespace: Some(namespace.into()),
        ..Default::default()
    })
    .unwrap();
    std::fs::write(
        &console.routes_file,
        serde_yaml::to_string(&document).unwrap(),
    )
    .unwrap();
    let mut config = support::test_config();
    config.console = console;
    config.storage_mode = crate::config::GatewayStorageMode::Server;
    config.database_url = Some(std::env::var("COMMANDCODE_TEST_DATABASE_URL").unwrap());
    config.redis_url = std::env::var("COMMANDCODE_TEST_REDIS_URL").unwrap();
    // Explicit loopback fixture identity prevents accidental use of a developer database.
    let database = url::Url::parse(config.database_url.as_ref().unwrap()).unwrap();
    assert_eq!(database.host_str(), Some("127.0.0.1"));
    assert!(matches!(
        database.path(),
        "/gateway_fixture" | "/gateway_fixture_unmigrated"
    ));
    assert_ne!(database.port(), Some(5432));
    assert_eq!(
        url::Url::parse(&config.redis_url).unwrap().host_str(),
        Some("127.0.0.1")
    );
    config.default_project_id = "platform-default-project".into();
    config.gateway_api_key = Some("server-public-synthetic".into());
    config.gateway_api_key_secret = None;
    config.gateway_management_token = Some("server-management-synthetic".into());
    let mut state = Box::pin(crate::runtime::build_app_state(config))
        .await
        .unwrap();
    assert!(state.local_runtime.is_none() && state.pg_pool.is_some());
    Arc::get_mut(&mut state).unwrap().upstream_client.http = peer.client();
    state
}

#[tokio::test]
#[ignore = "Requires explicitly provisioned isolated PostgreSQL and Redis; retains all data"]
async fn commandcode_server_db_two_states_native_send_audit_and_redis_slot() {
    let root = std::path::PathBuf::from(std::env::var("COMMANDCODE_TEST_EVIDENCE_ROOT").unwrap());
    assert!(root.starts_with("C:/Users/Public/nas_home/AI/GameEditor/linshi"));
    let namespace = format!("commandcode-server-{}", uuid::Uuid::new_v4().simple());
    let root = root.join(&namespace);
    let make_reply = || {
        let mut reply: Value =
            serde_json::from_slice(&native::reply("messages", false, false, true)).unwrap();
        // PostgreSQL enforces globally unique response IDs; each synthetic generation needs its own.
        reply["id"] = json!(format!("msg_{}", uuid::Uuid::new_v4().simple()));
        serde_json::to_vec(&reply).unwrap()
    };
    let peer = peer::Peer::raw(make_reply()).await;
    let second_peer = peer::Peer::raw(make_reply()).await;
    let first = state(&root.join("first"), &namespace, &peer).await;
    let account = call(&first, "/v1/internal/gateway/provider-accounts", "server-management-synthetic", json!({
        "label":namespace,"adapter":"openai_compatible","protocolFamily":"openai",
        "protocolProfile":"commandcode_chat","status":"active","executionMode":"direct_http",
        "payload":{"adapter":"openai_compatible","auth_mode":"commandcode_api_key",
            "base_url":super::super::BASE,"supported_models":[namespace],
            "model_catalog":{"sourceUrl":super::super::SOURCE,"checkedAt":"2026-11-14T00:00:00Z",
                "models":[namespace],"supportedEndpoints":{namespace.clone():["/messages"]}}}
    })).await;
    let account = account["providerAccount"]["id"].as_str().unwrap();
    let credential = call(&first, &format!("/v1/internal/gateway/provider-accounts/{account}/credentials"), "server-management-synthetic", json!({
        "label":"Synthetic exact credential","status":"active","sourceKind":"manual","syncMode":"manual",
        "payload":{"api_key":KEY,"supported_models":[namespace]}
    })).await;
    let credential = credential["providerCredential"]["id"].as_str().unwrap();
    let second = state(&root.join("second"), &namespace, &second_peer).await;
    for (current, peer) in [(&first, &peer), (&second, &second_peer)] {
        super::super::test_support::origin(peer.origin.clone(), async {
            let result = call(current, "/v1/messages", "server-public-synthetic", json!({
                "model":namespace,"max_tokens":32,"messages":[{"role":"user","content":"synthetic"}]
            })).await;
            assert_eq!(result["type"], "message");
        })
        .await;
    }
    for peer in [&peer, &second_peer] {
        let calls = peer.calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "one send per request, no replay");
        for (path, headers, body) in calls.iter() {
            assert_eq!(path, "/provider/v1/messages");
            assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
            assert_eq!(body["model"], namespace);
            assert!(!body.to_string().contains("model_catalog"));
        }
    }
    let pg = first.pg_pool.as_ref().unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let count: i64 = sqlx::query_scalar("select count(*) from gateway_request_audits where provider_account_id=$1 and status='completed' and total_tokens=5 and route_attempt_count=1")
                .bind(account).fetch_one(pg).await.unwrap();
            if count == 2 { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    let (a, b) = tokio::join!(
        crate::provider_runtime::test_store::claim(
            &first,
            account,
            credential,
            5,
            Some("messages")
        ),
        crate::provider_runtime::test_store::claim(
            &second,
            account,
            credential,
            5,
            Some("messages")
        )
    );
    assert_ne!(
        a.unwrap(),
        b.unwrap(),
        "exactly one shared Redis slot winner"
    );
    assert!(!crate::provider_runtime::test_store::claim(
        &second,
        account,
        credential,
        5,
        Some("messages")
    )
    .await
    .unwrap());
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(&json!({
            "passed":true,"providerAccountId":account,"credentialId":credential,
            "nativeMessagesCalls":2,"completedAudits":2,"sharedRedisSlot":true,
            "separateAppStates":true,"separateProcesses":false,"dataRetained":true
        }))
        .unwrap(),
    )
    .unwrap();
    for current in [first, second] {
        current.shutdown.request("commandcode-server-contract");
        current.pg_pool.as_ref().unwrap().close().await;
        current
            .upstream_client
            .freebuff
            .shutdown(Duration::from_secs(1))
            .await
            .unwrap();
    }
    peer.stop().await;
    second_peer.stop().await;
}
