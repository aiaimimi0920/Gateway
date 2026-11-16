//! Summary failures keep canonical history and reported obligations; no retry or hidden work call.
use super::completion::activate;
use super::multistep::response;
use super::root_summary::{canonical, scoped_account, summarized, turn, usage};
use super::*;

#[tokio::test]
async fn freebuff_root_summary_wire_invalid_and_truncated_replies_keep_history_and_original_costs()
{
    for truncated in [false, true] {
        let (root, db) = setup().await;
        let (session, device) = scoped_account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = activate(&db, &requests, &peer).await;
        let original = row(&db).await;
        let mut history = canonical();
        peer.push(fixture::Spec::json(200, json!({"runId":"failure-run"})));
        peer.push(fixture::Spec::json(200, usage()));
        let mut summary = response(Some(0.25));
        if truncated {
            summary["choices"][0]["message"]["content"] = json!(summarized());
            summary["choices"][0]["finish_reason"] = json!("length");
        }
        peer.push(fixture::Spec::json(200, summary));
        peer.push(fixture::Spec::json(204, json!(null)));
        with_test_origin(peer.origin.clone(), async {
            let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
            let (run, _) = run
                .execute_root_turn(turn(&mut history), 32768, BUDGET, std::future::pending())
                .await
                .continue_run()
                .unwrap();
            let before = history.messages().to_vec();
            let decoded = run
                .execute_root_turn(turn(&mut history), 32768, BUDGET, std::future::pending())
                .await;
            assert_eq!(
                decoded.completion().err(),
                Some(if truncated {
                    BusinessError::SummaryOutputLimit
                } else {
                    BusinessError::SummaryInvalid
                })
            );
            assert_eq!(history.messages(), before);
            assert_eq!(
                peer.count(),
                4,
                "one work request and one summary, no automatic retry/work"
            );
            let finished = decoded.finish_run(BUDGET).await.unwrap();
            assert_eq!(finished.totals.total_steps, 2);
            assert_eq!(finished.totals.total_credits, 39);
        })
        .await;
        assert_eq!(row(&db).await, original);
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}
#[tokio::test]
async fn freebuff_root_summary_wire_oversized_replacement_keeps_original_history_and_no_work_send()
{
    let (root, db) = setup().await;
    let (session, device) = scoped_account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let mut history = canonical();
    peer.push(fixture::Spec::json(200, json!({"runId":"large-run"})));
    peer.push(fixture::Spec::json(200, usage()));
    let mut summary = response(Some(0.25));
    summary["choices"][0]["message"]["content"] =
        json!(format!("{}{}", summarized(), "x".repeat(40000)));
    peer.push(fixture::Spec::json(200, summary));
    peer.push(fixture::Spec::json(204, json!(null)));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let (run, _) = run
            .execute_root_turn(turn(&mut history), 32768, BUDGET, std::future::pending())
            .await
            .continue_run()
            .unwrap();
        let before = history.messages().to_vec();
        let decoded = run
            .execute_root_turn(turn(&mut history), 32768, BUDGET, std::future::pending())
            .await;
        assert_eq!(
            decoded.completion().err(),
            Some(BusinessError::RequestTooLarge)
        );
        assert_eq!(history.messages(), before);
        assert_eq!(peer.count(), 4);
        assert_eq!(
            decoded
                .finish_run(BUDGET)
                .await
                .unwrap()
                .totals
                .total_credits,
            39
        );
    })
    .await;
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
#[tokio::test]
async fn freebuff_root_summary_wire_valid_swap_survives_work_failure_and_unknown_cost_is_not_zero()
{
    let (root, db) = setup().await;
    let (session, device) = scoped_account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let mut history = canonical();
    peer.push(fixture::Spec::json(
        200,
        json!({"runId":"work-failure-run"}),
    ));
    peer.push(fixture::Spec::json(200, usage()));
    let mut summary = response(Some(0.25));
    summary["choices"][0]["message"]["content"] = json!(summarized());
    peer.push(fixture::Spec::json(200, summary));
    peer.push(fixture::Spec::json(503, json!({"error":"synthetic"})));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let (run, _) = run
            .execute_root_turn(turn(&mut history), 32768, BUDGET, std::future::pending())
            .await
            .continue_run()
            .unwrap();
        let decoded = run
            .execute_root_turn(turn(&mut history), 32768, BUDGET, std::future::pending())
            .await;
        assert_eq!(
            decoded.completion().err(),
            Some(BusinessError::InvalidResponse)
        );
        assert_eq!(
            history.messages().len(),
            2,
            "validated compacted history, not a fabricated assistant response"
        );
        assert_eq!(history.messages()[1]["content"], "exact live request");
        assert_eq!(
            decoded.finish_run(BUDGET).await.err(),
            Some(BusinessError::AccountingUnknown)
        );
        assert_eq!(
            peer.count(),
            5,
            "no retry or zero-cost FINISH for the unknown work attempt"
        );
    })
    .await;
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
