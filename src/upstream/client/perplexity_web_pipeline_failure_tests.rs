//! Public refusal, no resubmission and cancellation use the ordinary finalization owners.
use super::*;

#[test]
fn perplexity_web_pipeline_auth_and_unsupported_requests_never_reach_transport() {
    run(rejections);
}
async fn rejections() {
    let server = wire::Server::start(wire::completed(), 200, false).await;
    let fixture = Fixture::new(&server).await;
    let (status, _) = fixture
        .request(
            &server,
            "chat/completions",
            body("chat/completions", false),
            false,
        )
        .await;
    assert!(!status.is_success());
    for (endpoint, request) in [
        (
            "messages",
            json!({"model":"public-alias","max_tokens":10,"messages":[{"role":"user","content":"hello"}]}),
        ),
        (
            "chat/completions",
            json!({"model":"public-alias","messages":[{"role":"user","content":"hello"}],"temperature":0.5}),
        ),
        (
            "chat/completions",
            json!({"model":"public-alias","messages":[{"role":"user","content":"hello"},{"role":"assistant","content":"old"}]}),
        ),
        (
            "responses",
            json!({"model":"public-alias","input":"hello","tools":[{"type":"web_search_preview"}]}),
        ),
        (
            "embeddings",
            json!({"model":"public-alias","input":"hello"}),
        ),
    ] {
        let (status, _) = fixture.request(&server, endpoint, request, true).await;
        assert!(!status.is_success(), "{endpoint} must be refused");
    }
    assert_eq!(server.attempts(), 0);
    fixture.finish().await;
}
#[test]
fn perplexity_web_pipeline_auth_rate_limit_and_incomplete_answers_are_never_retried() {
    run(failures);
}
async fn failures() {
    for (status, reply, code) in [
        (401, "synthetic-private-upstream".to_string(), "perplexity_web_authentication"),
        (429, "synthetic-private-upstream".to_string(), "perplexity_web_rate_limit"),
        (200, "data: {\"blocks\":[{\"intended_usage\":\"ask_text\",\"markdown_block\":{\"answer\":\"partial\"}}]}\n\n".to_string(), "perplexity_web_incomplete"),
    ] {
        let server = wire::Server::start(reply, status, false).await;
        let fixture = Fixture::new(&server).await;
        // Buffered errors still own an HTTP error; live post-header errors are checked separately.
        let (status, bytes) = fixture.request(&server, "responses", body("responses", false), true).await;
        assert!(!status.is_success());
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains(code), "{text}");
        assert!(!text.contains("synthetic-private-upstream"));
        assert_wire(&server);
        let rows = fixture.audits().await;
        assert_eq!(rows.len(), 1);
        assert_ne!(rows[0].status, "completed");
        assert!(rows[0].prompt_tokens.is_none() && rows[0].completion_tokens.is_none());
        fixture.finish().await;
    }
}
#[test]
fn perplexity_web_pipeline_cancelling_buffered_or_aggregating_stream_releases_audit_and_permit() {
    run(cancel);
}
async fn cancel() {
    for stream in [false, true] {
        let server = wire::Server::start(": keepalive\n\n", 200, true).await;
        let fixture = Fixture::new(&server).await;
        let controller = fixture
            .console
            .state
            .concurrency_registry
            .get_or_create("perplexity-pipeline");
        let initial_limit = controller.snapshot().current_limit;
        let mut request =
            Box::pin(fixture.request(&server, "responses", body("responses", stream), true));
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                tokio::select! {
                    _ = &mut request => panic!("Held upstream unexpectedly completed"),
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
        assert_eq!(controller.snapshot().active_count, 1);
        assert_eq!(server.closed(), 0);
        drop(request);
        let rows = fixture.audits().await;
        assert_eq!(rows[0].status, "cancelled");
        assert_eq!(
            rows[0].provider_account_id.as_deref(),
            Some("perplexity-pipeline")
        );
        assert_eq!(
            rows[0].route_trace.as_ref().unwrap()["realCredentialRef"],
            "exact"
        );
        assert!(rows[0].client_disconnected_at.is_none());
        assert!(rows[0].prompt_tokens.is_none() && rows[0].completion_tokens.is_none());
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
        assert_eq!(snapshot.current_limit, initial_limit);
        assert_eq!(snapshot.consecutive_successes, 0);
        assert!(fixture
            .local
            .list_model_states(db::CredentialModelStateFilters::default())
            .await
            .unwrap()
            .is_empty());
        fixture.finish().await;
    }
}
