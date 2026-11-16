//! Real SQLite/TCP root budget admission; no real credentials, tokenizer service or summary purchase.
use super::super::root_builder::tests::input;
use super::completion::activate;
use super::multistep::response;
use super::*;

#[tokio::test]
async fn freebuff_root_context_wire_catalog_budget_stops_model_send_and_retains_original_run() {
    let (root, db) = setup().await;
    let (mut session, device) = account();
    let mut catalog = tests::body();
    catalog["rows"][0]["compaction"] =
        json!({"cacheExpiryMs":3600000,"cacheExpiryMinTokens":140000,"maxContextLength":1});
    session.catalog = Some(ModelCatalog::parse(&serde_json::to_vec(&catalog).unwrap()).unwrap());
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"context-run"})));
    // This 200 is accepted by FINISH. In the red implementation it instead receives a model POST.
    peer.push(fixture::Spec::json(200, response(Some(0.25))));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let decoded = run
            .execute_root_step(input(), 32768, BUDGET, std::future::pending())
            .await;
        assert!(
            decoded.completion().is_err(),
            "the root must not exceed its catalog runtime budget"
        );
        assert_eq!(
            decoded.completion().err(),
            Some(BusinessError::ContextBudgetExceeded)
        );
        assert_eq!(decoded.context_forecast().unwrap().limit, 1);
        assert_eq!(
            peer.count(),
            2,
            "heartbeat and START only, no model attempt"
        );
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

fn budgeted(session: &mut CatalogSession, maximum: u64) {
    let mut catalog = tests::body();
    catalog["rows"][0]["compaction"] =
        json!({"cacheExpiryMs":3600000,"cacheExpiryMinTokens":140000,"maxContextLength":maximum});
    session.catalog = Some(ModelCatalog::parse(&serde_json::to_vec(&catalog).unwrap()).unwrap());
}
fn usage_spec(sse: bool) -> fixture::Spec {
    let mut body = response(Some(0.125));
    body["usage"]["prompt_tokens"] = json!(20000);
    body["usage"]["completion_tokens"] = json!(10);
    body["usage"]["prompt_tokens_details"] = json!({"cached_tokens":19000});
    let mut spec = fixture::Spec::json(200, body.clone());
    if sse {
        spec.headers
            .insert("content-type", "text/event-stream".parse().unwrap());
        // Last non-null usage is the real anchor, not the first observation or chunk sum.
        spec.body = format!("data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            json!({"choices":[{"index":0,"delta":{"role":"assistant","content":"answer"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1}}),
            json!({"choices":[],"usage":body["usage"]})).into_bytes();
    }
    spec
}

#[tokio::test]
async fn freebuff_root_context_wire_json_sse_usage_gates_next_step_without_losing_prior_costs() {
    for sse in [false, true] {
        let (root, db) = setup().await;
        let (mut session, device) = account();
        budgeted(&mut session, 10000);
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = activate(&db, &requests, &peer).await;
        let original = row(&db).await;
        peer.push(fixture::Spec::json(200, json!({"runId":"anchored-run"})));
        peer.push(usage_spec(sse));
        peer.push(fixture::Spec::json(204, json!(null)));
        with_test_origin(peer.origin.clone(), async {
            let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
            let first = run
                .execute_root_step(input(), 32768, BUDGET, std::future::pending())
                .await;
            assert!(!first.context_forecast().unwrap().calibrated);
            let (run, body) = first.continue_run().unwrap();
            let mut next = input();
            next.messages.extend([
                body["choices"][0]["message"].clone(),
                json!({"role":"user","content":"continue"}),
            ]);
            let rejected = run
                .execute_root_step(next, 32768, BUDGET, std::future::pending())
                .await;
            assert_eq!(
                rejected.completion().err(),
                Some(BusinessError::ContextBudgetExceeded)
            );
            let forecast = rejected.context_forecast().unwrap();
            assert!(forecast.calibrated);
            assert!(forecast.tokens >= 20010);
            assert!(forecast.estimated_tokens < forecast.limit);
            assert_eq!(peer.count(), 3, "the second model request is never sent");
            let finished = rejected.finish_run(BUDGET).await.unwrap();
            assert_eq!(finished.totals.total_steps, 1);
            assert_eq!(finished.totals.total_credits, 13);
        })
        .await;
        assert_eq!(row(&db).await, original);
        {
            let hits = peer.hits.lock().unwrap();
            assert_eq!(hits.len(), 4);
            let finish: Value = serde_json::from_slice(&hits[3].body).unwrap();
            assert_eq!(finish["runId"], "anchored-run");
            assert_eq!(finish["totalSteps"], 1);
            assert_eq!(
                finish["errorMessage"],
                "freebuff_business_context_budget_exceeded"
            );
            use base64::Engine;
            let public = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(device.key.public_key())
                .unwrap();
            business_tests::verify(&hits[2], &public, "synthetic-fetch");
        }
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}

#[tokio::test]
async fn freebuff_root_context_wire_rewritten_history_and_generic_harness_do_not_reuse_receipt() {
    let (root, db) = setup().await;
    let (mut session, device) = account();
    budgeted(&mut session, 10000);
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"rewrite-run"})));
    for _ in 0..4 {
        peer.push(usage_spec(false));
    }
    peer.push(fixture::Spec::json(204, json!(null)));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let (run, _) = run
            .execute_root_step(input(), 32768, BUDGET, std::future::pending())
            .await
            .continue_run()
            .unwrap();
        let mut rewritten = input();
        rewritten.messages[0]["content"] = json!("replacement");
        let decoded = run
            .execute_root_step(rewritten, 32768, BUDGET, std::future::pending())
            .await;
        assert!(!decoded.context_forecast().unwrap().calibrated);
        let (run, body) = decoded.continue_run().unwrap();
        let mut native = input();
        native.messages[0]["content"] = json!("replacement");
        native.messages.push(body["choices"][0]["message"].clone());
        let generic = run
            .execute_step(
                super::multistep::intent(),
                32768,
                BUDGET,
                std::future::pending(),
            )
            .await;
        assert!(generic.context_forecast().is_none());
        let (run, _) = generic.continue_run().unwrap();
        let decoded = run
            .execute_root_step(native, 32768, BUDGET, std::future::pending())
            .await;
        assert!(!decoded.context_forecast().unwrap().calibrated);
        let finished = decoded.finish_run(BUDGET).await.unwrap();
        assert_eq!(finished.totals.total_steps, 4);
        assert_eq!(finished.totals.total_credits, 52);
    })
    .await;
    assert_eq!(peer.count(), 7);
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
