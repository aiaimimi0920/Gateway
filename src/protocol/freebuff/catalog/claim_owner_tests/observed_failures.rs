//! A provisional delta cannot hide later native errors, observer projection failures or unknown spend.
use super::observed_stream::{facts, stream};
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn freebuff_catalog_observed_stream_invalid_later_frame_keeps_cost_before_failed_finish() {
    for malformed in [false, true] {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = completion::activate(&db, &requests, &peer).await;
        peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
        let mut spec = stream(
            vec![json!({"choices":[{"delta":{"content":"partial"}}],"usage":{"cost":0.125}})],
            false,
        );
        if malformed {
            spec.body.extend_from_slice(b"data: {bad}\n\n");
        } else {
            spec.body.extend(
                format!(
                    "data: {}\n\n",
                    json!({"choices":[{"delta":{"audio":{}}}],"usage":{"cost":0.25}})
                )
                .into_bytes(),
            );
        }
        peer.push(spec);
        peer.push(fixture::Spec::json(204, Value::Null));
        let calls = Arc::new(AtomicUsize::new(0));
        let observer = calls.clone();
        with_test_origin(peer.origin.clone(), async {
            let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
            let decoded = run
                .execute_step_observed(
                    multistep::intent(),
                    4096,
                    BUDGET,
                    std::future::pending(),
                    move |_| {
                        observer.fetch_add(1, Ordering::SeqCst);
                        std::future::ready(Ok(()))
                    },
                )
                .await;
            assert_eq!(
                decoded.completion().err(),
                Some(BusinessError::InvalidResponse)
            );
            assert_eq!(
                calls.load(Ordering::SeqCst),
                1,
                "bad frame is never delivered"
            );
            assert_eq!(
                facts(&db, &session).await["lastStep"]["cost"],
                if malformed { 0.125 } else { 0.25 }
            );
            let finished = decoded.finish_run(BUDGET).await.unwrap();
            assert_eq!(finished.totals.total_steps, 1);
            assert_eq!(
                finished.totals.total_credits,
                if malformed { 13 } else { 26 }
            );
        })
        .await;
        let finish: Value = serde_json::from_slice(&peer.hits.lock().unwrap()[3].body).unwrap();
        assert_eq!(finish["status"], "failed");
        assert_eq!(
            peer.count(),
            4,
            "one admission, original START/model/FINISH, no replay or end"
        );
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}

#[tokio::test]
async fn freebuff_catalog_observed_stream_observer_failure_returns_same_ledger_for_finish() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = completion::activate(&db, &requests, &peer).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    peer.push(stream(
        vec![
            json!({"choices":[{"delta":{"content":"partial"}}],"usage":{"cost":0.125}}),
            json!({"choices":[{"finish_reason":"stop"}],"usage":{"cost":0.25}}),
        ],
        true,
    ));
    peer.push(fixture::Spec::json(204, Value::Null));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let decoded = run
            .execute_step_observed(
                multistep::intent(),
                4096,
                BUDGET,
                std::future::pending(),
                |_| std::future::ready(Err(BusinessError::InvalidResponse)),
            )
            .await;
        assert_eq!(
            decoded.completion().err(),
            Some(BusinessError::InvalidResponse)
        );
        assert_eq!(facts(&db, &session).await["lastStep"]["cost"], 0.125);
        assert_eq!(
            decoded
                .continue_run()
                .err()
                .unwrap()
                .finish_run(BUDGET)
                .await
                .unwrap()
                .totals
                .total_credits,
            13
        );
    })
    .await;
    let finish: Value = serde_json::from_slice(&peer.hits.lock().unwrap()[3].body).unwrap();
    assert_eq!(finish["status"], "failed");
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_catalog_observed_stream_complete_without_report_never_fabricates_zero_finish() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = completion::activate(&db, &requests, &peer).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    peer.push(stream(
        vec![json!({"choices":[{"delta":{"content":"answer"},"finish_reason":"stop"}]})],
        false,
    ));
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let decoded = run
            .execute_step_observed(
                multistep::intent(),
                4096,
                BUDGET,
                std::future::pending(),
                move |_| {
                    observed.fetch_add(1, Ordering::SeqCst);
                    std::future::ready(Ok(()))
                },
            )
            .await;
        assert!(decoded.completion().is_ok(), "DONE is not newly required");
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(facts(&db, &session).await["totals"].is_null());
        assert_eq!(
            decoded.finish_run(BUDGET).await.err(),
            Some(BusinessError::AccountingUnknown)
        );
    })
    .await;
    assert_eq!(peer.count(), 3);
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_catalog_observed_stream_json_keeps_buffered_contract_without_fake_frames() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = completion::activate(&db, &requests, &peer).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    let answer = multistep::response(Some(0.125));
    peer.push(fixture::Spec::json(200, answer.clone()));
    peer.push(fixture::Spec::json(204, Value::Null));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let decoded = run
            .execute_step_observed(
                multistep::intent(),
                4096,
                BUDGET,
                std::future::pending(),
                |_| async {
                    panic!("buffered JSON is not an SSE delta");
                    #[allow(unreachable_code)]
                    Ok(())
                },
            )
            .await;
        assert_eq!(decoded.completion().unwrap(), &answer);
        assert_eq!(
            decoded
                .finish_run(BUDGET)
                .await
                .unwrap()
                .totals
                .total_credits,
            13
        );
    })
    .await;
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
