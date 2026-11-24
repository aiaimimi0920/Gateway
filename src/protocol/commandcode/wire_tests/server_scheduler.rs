//! Real periodic workers share Redis schedule/result authority; no clock or slot manipulation.
use super::*;
use crate::provider_credential_probe_scheduler::start_provider_credential_probe_scheduler;
use crate::provider_runtime::test_store;

pub(super) async fn close(state: Arc<AppState>) {
    state.shutdown.request("commandcode-server-scheduler");
    state.pg_pool.as_ref().unwrap().close().await;
    state
        .upstream_client
        .freebuff
        .shutdown(Duration::from_secs(1))
        .await
        .unwrap();
}

async fn latest_result(state: &AppState, account: &str) -> Option<Value> {
    let rows = test_store::read_many(state, account, &["scheduled"], None)
        .await
        .unwrap();
    if rows.is_empty() {
        return None;
    }
    assert_eq!(rows.len(), 1);
    let result = serde_json::to_value(&rows[0]).unwrap();
    assert_eq!(result["status"], "passed");
    assert_eq!(rows[0].assessment.as_ref().unwrap().mode, "automatic");
    assert_eq!(result["assessment"]["cases"][0]["correct"], true);
    Some(result)
}

#[test]
#[ignore = "Requires isolated PostgreSQL/Redis; real 60-second worker ticks; retains data"]
fn commandcode_server_scheduler_two_workers_real_ticks_and_restart() {
    run(real_ticks);
}

pub(super) fn run<F: std::future::Future<Output = ()> + 'static>(test: fn() -> F) {
    // Give Server bootstrap and probe futures the same bounded stack as the Local scheduler fixture.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(Box::pin(test()))
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
}

async fn real_ticks() {
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
    // A route-document provider is not a Hosted database account or a public caller.
    let account = namespace.as_str();
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
    let a = Box::pin(super::super::test_support::origin(
        peer.origin.clone(),
        start_provider_credential_probe_scheduler(first.clone()),
    ));
    let b = Box::pin(super::super::test_support::origin(
        peer.origin.clone(),
        start_provider_credential_probe_scheduler(second.clone()),
    ));
    let started = std::time::Instant::now();
    let observe = async {
        let mut first_checked_at = None;
        loop {
            let calls = peer.calls.lock().unwrap().len();
            assert!(calls <= 2, "workers sent more than two probes");
            if calls == 2 {
                assert!(
                    started.elapsed() >= Duration::from_secs(60),
                    "duplicate send in initial interval"
                );
            }
            // Wait for the second Redis publication, not a fixed delay after the audit commit.
            if let Some(saved) = latest_result(&first, account).await {
                let checked_at = saved["checkedAt"].as_str().unwrap();
                match &first_checked_at {
                    None => first_checked_at = Some(checked_at.to_owned()),
                    Some(initial) if initial != checked_at => {
                        assert_eq!(peer.calls.lock().unwrap().len(), 2);
                        first.shutdown.request("two-real-rounds-finished");
                        second.shutdown.request("two-real-rounds-finished");
                        break saved;
                    }
                    Some(_) => {}
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    let (_, _, saved) = tokio::time::timeout(Duration::from_secs(135), async {
        tokio::join!(a, b, observe)
    })
    .await
    .expect("two periodic sweeps did not publish results or stop");
    assert_eq!(peer.calls.lock().unwrap().len(), 2);
    assert_eq!(Some(&saved), latest_result(&second, account).await.as_ref());
    let audits = server_probe_audit::rows(&first, &namespace, account).await;
    assert_eq!(
        audits.len(),
        2,
        "each automatic probe requires one internal Postgres audit"
    );
    for audit in &audits {
        audit.assert_terminal("completed", Some((2, 3, 5)));
        assert_eq!(audit.credential_id, "scheduled");
        assert_eq!(audit.model, namespace);
    }
    server_probe_audit::assert_no_public_audits(&first, &namespace).await;
    close(first).await;
    close(second).await;
    let restarted =
        server::state_document(&root.join("restart"), &namespace, &peer, document).await;
    assert_eq!(
        Some(&saved),
        latest_result(&restarted, account).await.as_ref()
    );
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
    assert_eq!(
        audits,
        server_probe_audit::rows(&restarted, &namespace, account).await
    );
    close(restarted).await;
    peer.stop().await;

    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(&json!({
            "passed":true,"realPeriodicWorkers":2,"generatedCalls":2,"completedInternalAudits":audits.len(),"publicAudits":0,
            "sharedResults":true,"restartPreservesResultAndSlot":true,"dataRetained":true
        }))
        .unwrap(),
    )
    .unwrap();
}
