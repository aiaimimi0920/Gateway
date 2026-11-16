//! Real periodic workers share Redis schedule/result authority; no clock or slot manipulation.
use super::*;
use crate::provider_credential_probe_scheduler::start_provider_credential_probe_scheduler;
use crate::provider_runtime::test_store;

async fn close(state: Arc<AppState>) {
    state.shutdown.request("commandcode-server-scheduler");
    state.pg_pool.as_ref().unwrap().close().await;
    state
        .upstream_client
        .freebuff
        .shutdown(Duration::from_secs(1))
        .await
        .unwrap();
}

async fn results(state: &AppState, account: &str) -> Value {
    let rows = test_store::read_many(state, account, &["scheduled"], None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let result = serde_json::to_value(&rows[0]).unwrap();
    assert_eq!(result["status"], "passed");
    assert_eq!(rows[0].assessment.as_ref().unwrap().mode, "automatic");
    result
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL/Redis; real 60-second worker ticks; retains data"]
async fn commandcode_server_scheduler_two_workers_real_ticks_and_restart() {
    let root = std::path::PathBuf::from(std::env::var("COMMANDCODE_TEST_EVIDENCE_ROOT").unwrap());
    assert!(root.starts_with("C:/Users/Public/nas_home/AI/GameEditor/linshi"));
    let namespace = format!("commandcode-scheduler-{}", uuid::Uuid::new_v4().simple());
    let root = root.join(&namespace);
    let peer = peer::Peer::raw(
        serde_json::to_vec(&json!({
            "id":"scheduler-native","model":"synthetic","choices":[{"index":0,
            "message":{"role":"assistant","content":"合成答案"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}
        }))
        .unwrap(),
    )
    .await;
    // Keep the real provider FK. Route-document scheduling and Hosted account routing have
    // different authorities; this database account only supplies the audit identity.
    let bootstrap = server::state(
        &root.join("bootstrap"),
        &format!("{namespace}-bootstrap"),
        &peer,
    )
    .await;
    let created = server::call(&bootstrap, "/v1/internal/gateway/provider-accounts", "server-management-synthetic", json!({
        "label":namespace,"adapter":"openai_compatible","protocolFamily":"openai","protocolProfile":"commandcode_chat",
        "payload":{"adapter":"openai_compatible","auth_mode":"commandcode_api_key","base_url":super::super::BASE}
    })).await;
    let account = created["providerAccount"]["id"].as_str().unwrap();
    close(bootstrap).await;
    let document = json!({"providers":[{"id":account,"preset":super::super::PRESET,
        "base_url":super::super::BASE,"explicit_credentials_only":true,"credentials":[{
            "id":"scheduled","api_key":KEY,"supported_models":[namespace],"test_policy":{
                "automaticEnabled":true,"intervalMinutes":1,"modelSelection":"selected","models":[namespace],
                "cases":[{"id":"text","name":"Text","prompt":"only OK","expectedAnswer":"合成答案","difficulty":1,"enabled":true}]}
        }]}],"model_routes":[]});
    let first =
        server::state_document(&root.join("first"), &namespace, &peer, document.clone()).await;
    let second =
        server::state_document(&root.join("second"), &namespace, &peer, document.clone()).await;
    assert_eq!(
        first
            .route_config
            .snapshot()
            .scheduled_credential_probe_targets()
            .len(),
        1
    );
    assert_eq!(
        second
            .route_config
            .snapshot()
            .scheduled_credential_probe_targets()
            .len(),
        1
    );
    let a = super::super::test_support::origin(
        peer.origin.clone(),
        start_provider_credential_probe_scheduler(first.clone()),
    );
    let b = super::super::test_support::origin(
        peer.origin.clone(),
        start_provider_credential_probe_scheduler(second.clone()),
    );
    let observe = async {
        loop {
            let count: i64 = sqlx::query_scalar("select count(*) from gateway_request_audits where provider_account_id=$1 and status='completed' and total_tokens=5")
                .bind(account).fetch_one(first.pg_pool.as_ref().unwrap()).await.unwrap();
            assert!(count <= 2, "workers duplicated the same schedule interval");
            if count == 2 {
                // Terminal audit precedes Redis result persistence; let each completed sweep settle.
                tokio::time::sleep(Duration::from_millis(200)).await;
                first.shutdown.request("two-real-rounds-finished");
                second.shutdown.request("two-real-rounds-finished");
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(135), async {
        tokio::join!(a, b, observe);
    })
    .await
    .unwrap();
    assert_eq!(peer.calls.lock().unwrap().len(), 2);
    let saved = results(&first, account).await;
    assert_eq!(saved, results(&second, account).await);
    close(first).await;
    close(second).await;
    let restarted =
        server::state_document(&root.join("restart"), &namespace, &peer, document).await;
    assert_eq!(saved, results(&restarted, account).await);
    assert!(
        !test_store::claim(&restarted, account, "scheduled", 1, None)
            .await
            .unwrap(),
        "rebuild must retain the live Redis slot"
    );
    {
        let calls = peer.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        for (path, headers, body) in calls.iter() {
            assert_eq!(path, "/provider/v1/chat/completions");
            assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
            assert_eq!(body["model"], namespace);
        }
    }
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(&json!({
            "passed":true,"realPeriodicWorkers":2,"generatedCalls":2,"completedAudits":2,
            "sharedResults":true,"restartPreservesResultAndSlot":true,"dataRetained":true
        }))
        .unwrap(),
    )
    .unwrap();
    close(restarted).await;
    peer.stop().await;
}
