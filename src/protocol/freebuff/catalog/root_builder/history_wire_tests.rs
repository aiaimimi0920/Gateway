//! Real signed wire verifies repair is consumed before the final original-run byte freeze.
use super::super::root_builder::tests::input;
use super::completion::activate;
use super::multistep::response;
use super::*;

fn call(id: &str, argument: Value) -> Value {
    json!({"id":id,"type":"function","function":{"name":"read_files","arguments":argument}})
}
#[tokio::test]
async fn freebuff_root_history_wire_repaired_native_snapshot_is_signed_on_original_run() {
    let (root, db) = setup().await;
    let (mut session, device) = account();
    let mut catalog = tests::body();
    catalog["fetchId"] = json!("history-fetch");
    session.catalog = Some(ModelCatalog::parse(&serde_json::to_vec(&catalog).unwrap()).unwrap());
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"history-run"})));
    peer.push(fixture::Spec::json(200, response(Some(0.25))));
    peer.push(fixture::Spec::json(204, json!(null)));
    let answered = call("answered", json!("{}"));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let mut step = input();
        step.messages = vec![
            json!({"role":"user","content":"start"}),
            json!({"role":"assistant","content":null,"reasoning_content":"keep", "reasoning_details":[{"signature":"synthetic"}]}),
            json!({"role":"assistant","content":"working", "tool_calls":[answered.clone(),call("late",json!("{}"))]}),
            json!({"role":"tool","tool_call_id":"answered","content":"answer"}),
            json!({"role":"user","content":"next"}),
            json!({"role":"tool","tool_call_id":"late","content":"freestanding late result"}),
        ];
        let finished = run.execute_root_step(step, 32768, BUDGET, std::future::pending())
            .await.finish_run(BUDGET).await.unwrap();
        assert!(finished.completion.is_ok());
        assert_eq!(finished.totals.total_steps, 1);
        assert_eq!(finished.totals.total_credits, 26);
    }).await;
    assert_eq!(row(&db).await, original);
    {
        let hits = peer.hits.lock().unwrap();
        assert_eq!(
            hits.len(),
            4,
            "heartbeat / START / one model step / explicit FINISH"
        );
        let body: Value = serde_json::from_slice(&hits[2].body).unwrap();
        assert_eq!(body["model"], "fbm1.synthetic-open");
        assert_eq!(body["codebuff_metadata"]["run_id"], "history-run");
        assert_eq!(body["messages"].as_array().unwrap().len(), 6);
        assert_eq!(
            body["messages"][2],
            json!({"role":"assistant","content":"working",
            "reasoning_content":"keep","reasoning_details":[{"signature":"synthetic"}],"tool_calls":[answered]})
        );
        assert_eq!(body["messages"][5]["content"], "freestanding late result");
        use base64::Engine;
        let public = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(device.key.public_key())
            .unwrap();
        business_tests::verify(&hits[2], &public, "history-fetch");
        let finish: Value = serde_json::from_slice(&hits[3].body).unwrap();
        assert_eq!(finish["runId"], "history-run");
        assert_eq!(finish["totalSteps"], 1);
    }
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}

async fn rejected(oversized: bool) {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"history-run"})));
    peer.push(fixture::Spec::json(204, json!(null)));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let mut step = input();
        if oversized {
            step.messages.push(json!({"role":"assistant","content":null,
                "tool_calls":[call("will-be-dropped",json!("x".repeat(32768)))]}));
        } else {
            step.messages
                .push(json!({"role":"assistant","tool_calls":"malformed"}));
        }
        let decoded = run
            .execute_root_step(step, 32768, BUDGET, std::future::pending())
            .await;
        assert_eq!(
            decoded.completion().err(),
            Some(if oversized {
                BusinessError::RequestTooLarge
            } else {
                BusinessError::InvalidInput
            })
        );
        assert_eq!(peer.count(), 2, "no model attempt");
        let finished = decoded.finish_run(BUDGET).await.unwrap();
        assert_eq!(finished.totals.total_steps, 0);
        assert_eq!(finished.totals.total_credits, 0);
    })
    .await;
    assert_eq!(row(&db).await, original);
    assert_eq!(peer.count(), 3);
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
#[tokio::test]
async fn freebuff_root_history_wire_malformed_history_returns_owner_without_model_send() {
    rejected(false).await;
}
#[tokio::test]
async fn freebuff_root_history_wire_budget_checked_even_when_huge_unanswered_call_would_be_dropped()
{
    rejected(true).await;
}
