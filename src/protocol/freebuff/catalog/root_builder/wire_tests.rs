//! Real signed TCP/SQLite request assembly, not local tool execution or automatic compaction.
use super::super::root_builder::tests::input;
use super::completion::activate;
use super::multistep::response;
use super::*;

#[tokio::test]
async fn freebuff_root_builder_wire_two_steps_keep_catalog_claim_and_original_signed_run() {
    let (root, db) = setup().await;
    let (mut session, device) = account();
    let mut catalog = tests::body();
    catalog["rows"][0]["handle"] = json!("fbm1.root-rotated");
    catalog["rows"][0]["displayName"] = json!("host-visible-label");
    catalog["rows"][0]["legacyDigests"] = json!(["1e303ac563a6f9cc"]);
    catalog["fetchId"] = json!("root-fetch");
    session.catalog = Some(ModelCatalog::parse(&serde_json::to_vec(&catalog).unwrap()).unwrap());
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"root-run"})));
    peer.push(fixture::Spec::json(200, response(Some(0.125))));
    peer.push(fixture::Spec::json(200, response(Some(0.25))));
    peer.push(fixture::Spec::json(204, json!(null)));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let decoded = run
            .execute_root_step(input(), 32768, BUDGET, std::future::pending())
            .await;
        let (run, answer) = decoded.continue_run().unwrap();
        let mut next = input();
        next.messages.extend([
            answer["choices"][0]["message"].clone(),
            json!({"role":"user","content":"second"}),
        ]);
        let finished = run
            .execute_root_step(next, 32768, BUDGET, std::future::pending())
            .await
            .finish_run(BUDGET)
            .await
            .unwrap();
        assert!(finished.completion.is_ok());
        assert_eq!(finished.totals.total_steps, 2);
        assert_eq!(finished.totals.total_credits, 39);
    })
    .await;
    assert_eq!(row(&db).await, original);
    {
        let hits = peer.hits.lock().unwrap();
        assert_eq!(hits.len(), 5);
        let first: Value = serde_json::from_slice(&hits[2].body).unwrap();
        let second: Value = serde_json::from_slice(&hits[3].body).unwrap();
        assert_eq!(first["model"], "fbm1.root-rotated");
        assert_eq!(first["provider"], json!({"data_collection":"deny"}));
        assert_eq!(first["codebuff_metadata"], second["codebuff_metadata"]);
        assert_eq!(second["codebuff_metadata"]["run_id"], "root-run");
        assert_eq!(first["messages"][0], second["messages"][0]);
        assert!(first["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("host-visible-label"));
        assert_eq!(first["messages"].as_array().unwrap().len(), 3);
        assert_eq!(second["messages"].as_array().unwrap().len(), 5);
        assert_eq!(first["messages"][2], second["messages"][4]);
        assert_eq!(second["messages"][3]["content"], "second");
        use base64::Engine;
        let public = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(device.key.public_key())
            .unwrap();
        business_tests::verify(&hits[2], &public, "root-fetch");
        business_tests::verify(&hits[3], &public, "root-fetch");
        let finish: Value = serde_json::from_slice(&hits[4].body).unwrap();
        assert_eq!(finish["runId"], "root-run");
        assert_eq!(finish["totalSteps"], 2);
    }
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
async fn rejected(phase: u8) {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"root-run"})));
    peer.push(fixture::Spec::json(204, json!(null)));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let mut step = input();
        if phase != 2 {
            step.tools.clear();
        }
        let limit = if phase == 2 { 64 } else { 32768 };
        let cancelled = async move {
            if phase == 1 {
                return;
            }
            std::future::pending::<()>().await;
        };
        let decoded = run.execute_root_step(step, limit, BUDGET, cancelled).await;
        let error = match phase {
            0 => BusinessError::InvalidInput,
            1 => BusinessError::Cancelled,
            _ => BusinessError::RequestTooLarge,
        };
        assert_eq!(decoded.completion().err(), Some(error));
        assert_eq!(peer.count(), 2);
        let finish = decoded.finish_run(BUDGET).await.unwrap();
        assert_eq!(finish.totals.total_steps, 0);
        assert_eq!(finish.totals.total_credits, 0);
    })
    .await;
    assert_eq!(row(&db).await, original);
    assert_eq!(
        peer.count(),
        3,
        "heartbeat/START/explicit FINISH only; no model attempt"
    );
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
#[tokio::test]
async fn freebuff_root_builder_wire_missing_tools_returns_original_run_without_send() {
    rejected(0).await;
}
#[tokio::test]
async fn freebuff_root_builder_wire_precancelled_does_not_even_validate_or_render() {
    rejected(1).await;
}
#[tokio::test]
async fn freebuff_root_builder_wire_final_serializer_limit_retains_zero_attempt_owner() {
    rejected(2).await;
}
