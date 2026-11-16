//! A sequential tool exchange reuses the original signed run, not the legacy zero-cost lease.
use super::completion::{activate, answer};
use super::*;

pub(super) fn intent() -> Value {
    json!({"model":"fbm1.synthetic-open","messages":[{"role":"user","content":"synthetic"}],
        "tools":[{"type":"function","function":{"name":"lookup","parameters":{"type":"object"}}}]})
}
pub(super) fn response(cost: Option<f64>) -> Value {
    let mut body = answer();
    body["usage"] = json!({"cost":cost});
    body
}
#[tokio::test]
async fn freebuff_catalog_multistep_original_run_signatures_tool_history_and_per_step_costs() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    let mut first = response(Some(0.125));
    first["choices"][0]["message"] = json!({"role":"assistant","content":null,
        "tool_calls":[{"id":"lookup-1","type":"function","function":{"name":"lookup","arguments":"{\"query\":\"synthetic\"}"}}]});
    first["choices"][0]["finish_reason"] = json!("tool_calls");
    peer.push(fixture::Spec::json(200, first));
    let mut second = fixture::Spec::json(200, json!(null));
    second
        .headers
        .insert("content-type", "text/event-stream".parse().unwrap());
    second.body = format!(
        "data: {}\n\ndata: [DONE]\n\n",
        json!({"choices":[{"index":0,
        "delta":{"content":"answer"},"finish_reason":"stop"}],"usage":{"cost":0.25}})
    )
    .into_bytes();
    peer.push(second);
    peer.push(fixture::Spec::json(204, json!(null)));
    let finished = with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let decoded = run
            .prepare_completion(intent(), 4096)
            .unwrap()
            .send(BUDGET)
            .await
            .unwrap()
            .decode(BUDGET)
            .await;
        let (run, completion) = decoded.continue_run().unwrap();
        let mut next = intent();
        next["messages"].as_array_mut().unwrap().extend([
            completion["choices"][0]["message"].clone(),
            json!({"role":"tool","tool_call_id":"lookup-1","content":"synthetic result"}),
        ]);
        run.prepare_completion(next, 4096)
            .unwrap()
            .send(BUDGET)
            .await
            .unwrap()
            .decode(BUDGET)
            .await
            .finish_run(BUDGET)
            .await
            .unwrap()
    })
    .await;
    assert!(finished.completion.is_ok());
    assert_eq!(
        row(&db).await,
        original,
        "FINISH does not clear the durable session"
    );
    {
        let hits = peer.hits.lock().unwrap();
        assert_eq!(
            hits.len(),
            5,
            "one heartbeat, one START, two completions, one FINISH"
        );
        let finish: Value = serde_json::from_slice(&hits[4].body).unwrap();
        assert_eq!(finish["totalSteps"], 2);
        assert_eq!(
            finish["directCredits"], 39,
            "round each model's reported credits before adding, not aggregate dollars"
        );
        assert_eq!(finish["totalCredits"], 39);
        assert_eq!(finish["runId"], "original-run");
        let a: Value = serde_json::from_slice(&hits[2].body).unwrap();
        let b: Value = serde_json::from_slice(&hits[3].body).unwrap();
        assert_eq!(a["codebuff_metadata"], b["codebuff_metadata"]);
        assert_eq!(a["codebuff_metadata"]["run_id"], "original-run");
        assert_eq!(b["messages"][1]["tool_calls"][0]["id"], "lookup-1");
        assert_eq!(b["messages"][2]["tool_call_id"], "lookup-1");
        use base64::Engine;
        let public = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(device.key.public_key())
            .unwrap();
        business_tests::verify(&hits[2], &public, "synthetic-fetch");
        business_tests::verify(&hits[3], &public, "synthetic-fetch");
    }
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
