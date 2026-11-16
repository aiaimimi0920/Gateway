//! Cooperatively abort the original summary owner at known wire phases; no detached task or retry.
use super::completion::activate;
use super::root_summary::{canonical, scoped_account, summarized, turn, usage};
use super::*;

#[tokio::test]
async fn freebuff_root_summary_wire_cancel_and_timeout_after_report_keep_history_and_ledger() {
    for timeout in [false, true] {
        let (root, db) = setup().await;
        let (session, device) = scoped_account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = activate(&db, &requests, &peer).await;
        let original = row(&db).await;
        let mut history = canonical();
        peer.push(fixture::Spec::json(200, json!({"runId":"cancel-run"})));
        peer.push(fixture::Spec::json(200, usage()));
        let mut summary = fixture::Spec::json(200, json!(null));
        summary
            .headers
            .insert("content-type", "text/event-stream".parse().unwrap());
        summary.body = format!("data: {}\n\ndata: [DONE]\n\n",json!({"choices":[{"index":0,"delta":{"role":"assistant","content":summarized()},"finish_reason":"stop"}],"usage":{"cost":0.25}})).into_bytes();
        summary.eof_delay = Duration::from_millis(600);
        peer.push(summary);
        peer.push(fixture::Spec::json(204, json!(null)));
        with_test_origin(peer.origin.clone(), async {
            let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
            let (run, _) = run
                .execute_root_turn(turn(&mut history), 32768, BUDGET, std::future::pending())
                .await
                .continue_run()
                .unwrap();
            let before = history.messages().to_vec();
            let seen = Arc::new(tokio::sync::Notify::new());
            let cancellation = async {
                if timeout {
                    std::future::pending::<()>().await
                } else {
                    seen.notified().await
                }
            };
            let decoded = TEST_BODY_STARTED
                .scope(
                    seen.clone(),
                    run.execute_root_turn(
                        turn(&mut history),
                        32768,
                        if timeout {
                            Duration::from_millis(250)
                        } else {
                            BUDGET
                        },
                        cancellation,
                    ),
                )
                .await;
            assert_eq!(
                decoded.completion().err(),
                Some(if timeout {
                    BusinessError::Timeout
                } else {
                    BusinessError::Cancelled
                })
            );
            assert_eq!(
                history.messages(),
                before,
                "not even [DONE] commits before real EOF"
            );
            assert_eq!(peer.count(), 4);
            let finished = decoded.finish_run(BUDGET).await.unwrap();
            assert_eq!(finished.totals.total_steps, 2);
            assert_eq!(finished.totals.total_credits, 39);
        })
        .await;
        assert_eq!(row(&db).await, original);
        peer.wait_drops(5).await;
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}
#[tokio::test]
async fn freebuff_root_summary_wire_precancel_is_local_and_summary_header_cancel_is_unknown() {
    for precancel in [false, true] {
        let (root, db) = setup().await;
        let (session, device) = scoped_account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = activate(&db, &requests, &peer).await;
        let mut history = canonical();
        peer.push(fixture::Spec::json(200, json!({"runId":"headers-run"})));
        peer.push(fixture::Spec::json(200, usage()));
        if !precancel {
            let mut spec = fixture::Spec::json(200, json!({}));
            spec.header_delay = Duration::from_millis(150);
            peer.push(spec);
        } else {
            peer.push(fixture::Spec::json(204, json!(null)));
        }
        with_test_origin(peer.origin.clone(), async {
            let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
            let (run, _) = run
                .execute_root_turn(turn(&mut history), 32768, BUDGET, std::future::pending())
                .await
                .continue_run()
                .unwrap();
            let before = history.messages().to_vec();
            let decoded = run
                .execute_root_turn(turn(&mut history), 32768, BUDGET, async {
                    if !precancel {
                        while peer.count() < 4 {
                            tokio::task::yield_now().await;
                        }
                    }
                })
                .await;
            assert_eq!(decoded.completion().err(), Some(BusinessError::Cancelled));
            assert_eq!(history.messages(), before);
            if precancel {
                assert_eq!(peer.count(), 3, "no summary preparation/send");
                let finished = decoded.finish_run(BUDGET).await.unwrap();
                assert_eq!(finished.totals.total_steps, 1);
                assert_eq!(finished.totals.total_credits, 13);
            } else {
                assert_eq!(
                    decoded.finish_run(BUDGET).await.err(),
                    Some(BusinessError::AccountingUnknown)
                );
                assert_eq!(
                    peer.count(),
                    4,
                    "unknown summary has no guessed-zero FINISH"
                );
            }
        })
        .await;
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}
