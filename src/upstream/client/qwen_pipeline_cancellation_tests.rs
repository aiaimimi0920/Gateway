//! Dropping a real public buffered request releases generation and records neutral cancellation.
use super::*;

#[test]
fn qwen_pipeline_buffered_outer_drop_finalizes_exact_audit_without_usage_or_health() {
    run(cancel);
}
async fn cancel() {
    for endpoint in ["chat/completions", "responses"] {
        let server = wire::Server::hold_json("{\"choices\":".into()).await;
        let fixture = Fixture::new(&server).await;
        let controller = fixture
            .console
            .state
            .concurrency_registry
            .get_or_create("qwen-pipeline");
        let original_limit = controller.snapshot().current_limit;
        let mut request = Box::pin(fixture.request(&server, endpoint, true));
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                tokio::select! {
                    _ = &mut request => panic!("Held buffered generation unexpectedly completed"),
                    _ = tokio::time::sleep(Duration::from_millis(5)) => {}
                }
                let rows = fixture
                    .local
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                if server.calls().len() == 1 && rows.len() == 1 && rows[0].status == "running" {
                    break;
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(server.closed(), 0);
        assert_eq!(controller.snapshot().active_count, 1);
        drop(request);
        let rows = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let rows = fixture
                    .local
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                if rows.len() == 1 && rows[0].status == "cancelled" {
                    break rows;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("Dropped public buffered request left its audit running");
        let row = &rows[0];
        assert_eq!(row.provider_account_id.as_deref(), Some("qwen-pipeline"));
        assert_eq!(row.resolved_model.as_deref(), Some("qwen-synthetic"));
        assert_eq!(
            row.route_trace.as_ref().unwrap()["realCredentialRef"],
            "exact"
        );
        assert!(row.completed_at.is_some());
        assert!(
            row.client_disconnected_at.is_none(),
            "Drop does not prove client disconnect"
        );
        assert!(
            row.prompt_tokens.is_none()
                && row.completion_tokens.is_none()
                && row.total_tokens.is_none()
        );
        assert!(fixture
            .local
            .list_model_states(db::CredentialModelStateFilters::default())
            .await
            .unwrap()
            .is_empty());
        tokio::time::timeout(Duration::from_secs(3), async {
            while server.closed() == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_wire(&server);
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.active_count, 0);
        assert_eq!(snapshot.current_limit, original_limit);
        assert_eq!(snapshot.consecutive_successes, 0);
        fixture.finish().await;
        server.stop().await;
    }
}
