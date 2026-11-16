//! Provisional frames cross the observer before EOF, after the original SQLite cost observation.
use super::*;
pub(super) fn stream(values: Vec<Value>, done: bool) -> fixture::Spec {
    let mut spec = fixture::Spec::json(200, Value::Null);
    spec.headers
        .insert("content-type", "text/event-stream".parse().unwrap());
    spec.body = values
        .into_iter()
        .flat_map(|v| format!("data: {v}\n\n").into_bytes())
        .collect();
    if done {
        spec.body.extend_from_slice(b"data: [DONE]\n\n");
    }
    spec
}
pub(super) async fn facts(db: &LocalRuntime, session: &CatalogSession) -> Value {
    serde_json::to_value(
        db.freebuff_claim_journal("pool", "account", &session.stored_claim_scope(), None)
            .await
            .unwrap(),
    )
    .unwrap()["claim"]["run"]
        .clone()
}
fn sendable<T: Send>(value: T) -> T {
    value
}

#[tokio::test]
async fn freebuff_catalog_observed_stream_delivers_durable_frames_before_real_eof() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = completion::activate(&db, &requests, &peer).await;
    let first = json!({"id":"original-answer","model":"reported-model","choices":[{"index":0,
        "delta":{"content":"你","reasoning_content":"native thought","reasoning_details":[{"opaque":1}]}}],"usage":{"cost":0.125}});
    let second = json!({"choices":[{"index":0,"delta":{"content":"好"},"finish_reason":"stop"}],"usage":{"cost":0.25}});
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    let mut spec = stream(vec![first.clone(), second.clone()], true);
    let eof = spec.hold_eof();
    peer.push(spec);
    peer.push(fixture::Spec::json(204, Value::Null));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let resume = Arc::new(tokio::sync::Notify::new());
        let pause = resume.clone();
        let mut input = multistep::intent();
        input["stream"] = json!(true);
        let step = sendable(run.execute_step_observed(input, 4096, BUDGET, std::future::pending(), move |frame| {
            let tx = tx.clone(); let pause = pause.clone();
            async move {
                tx.send(frame).await.map_err(|_| BusinessError::Cancelled)?;
                pause.notified().await;
                Ok(())
            }
        }));
        tokio::pin!(step);
        let observed = tokio::select! {
            result = &mut step => panic!("frame was not delivered before real EOF: {:?}", result.completion()),
            event = rx.recv() => event.unwrap(),
        };
        assert_eq!(observed, first);
        let saved = facts(&db, &session).await;
        assert_eq!(saved["phase"], "model_observed");
        assert_eq!(saved["lastStep"]["cost"], 0.125);
        assert_eq!(saved["totals"]["totalCredits"], 13);
        resume.notify_one();
        let observed = tokio::select! {
            _ = &mut step => panic!("finish frame is not full completion"),
            event = rx.recv() => event.unwrap(),
        };
        assert_eq!(observed, second);
        assert_eq!(facts(&db, &session).await["lastStep"]["cost"], 0.25);
        resume.notify_one();
        tokio::select! { biased; _ = &mut step => panic!("DONE cannot replace real EOF"), _ = std::future::ready(()) => {} }
        assert_eq!(peer.count(), 3, "observer and DONE cannot send FINISH");
        eof.send(()).unwrap();
        let decoded = step.await;
        assert_eq!(decoded.completion().unwrap()["choices"][0]["message"]["content"], "你好");
        assert_eq!(decoded.completion().unwrap()["choices"][0]["message"]["reasoning_details"], json!([{"opaque":1}]));
        assert_eq!(facts(&db, &session).await["phase"], "step_stopped");
        let finished = decoded.finish_run(BUDGET).await.unwrap();
        assert_eq!(finished.totals.total_steps, 1);
        assert_eq!(finished.totals.total_credits, 26);
        assert_eq!(facts(&db, &session).await["phase"], "finish_accepted");
    }).await;
    let hits = peer.hits.lock().unwrap();
    assert_eq!(hits.len(), 4);
    let native: Value = serde_json::from_slice(&hits[2].body).unwrap();
    assert_eq!(native["stream"], true);
    assert_eq!(native["codebuff_metadata"]["run_id"], "original-run");
    let finish: Value = serde_json::from_slice(&hits[3].body).unwrap();
    assert_eq!(finish["status"], "completed");
    drop(hits);
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
