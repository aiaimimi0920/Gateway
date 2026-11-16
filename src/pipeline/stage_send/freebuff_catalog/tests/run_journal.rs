//! Normal public generation and management read the same durable run, never a recovery executor.
use super::*;
pub(super) const JOURNAL: &str = "/v1/internal/gateway/console/freebuff-session-claim";

pub(super) async fn view(host: &Host, grant: &str) -> Value {
    let (status, value) = host.management(JOURNAL, host.input(), Some(grant)).await;
    assert_eq!(status, 200, "{value}");
    value
}

#[tokio::test]
async fn freebuff_public_run_journal_preserves_original_finish_and_archived_facts() {
    let peer = peer::Peer::new().await;
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let grant = host.grant().await;
    let task = host.public(request());
    let pending = host.pending(&grant, &task).await;
    assert_eq!(host.decision(&grant, &pending, "approve").await, 200);
    assert_eq!(task.await.unwrap().0, 200);
    let journal = view(&host, &grant).await;
    let run = &journal["claim"]["run"];
    assert_eq!(run["phase"], "finish_accepted");
    assert_eq!(run["runIdRecorded"], true);
    assert_eq!(run["totals"]["totalSteps"], 1);
    assert_eq!(run["totals"]["totalCredits"], 13);
    assert_eq!(run["lastStep"]["cost"], 0.1);
    assert_eq!(run["lastStep"]["upstreamEstimate"], 0.02);
    assert_eq!(run["finishStatus"], "completed");
    assert_eq!(run["finishHttpStatus"], 200);
    for secret in [
        "private prompt",
        "synthetic-account",
        "runId\"",
        "clientId",
        "sealed",
    ] {
        assert!(!journal.to_string().contains(secret), "{secret}");
    }
    let mut end = host.input();
    end["attemptId"] = journal["claim"]["attemptId"].clone();
    let (status, _) = host
        .management(&format!("{JOURNAL}/end"), end, Some(&grant))
        .await;
    assert_eq!(status, 200);
    let closed = view(&host, &grant).await;
    assert!(closed["claim"].is_null());
    assert_eq!(closed["history"][0]["run"], *run);
    assert_eq!(peer.finishes().len(), 1);
    assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 1);
    host.close().await;
    peer.server.close().await;
}

#[tokio::test]
async fn freebuff_public_run_journal_storage_failure_blocks_next_wire_without_retry() {
    for phase in ["before_start", "before_finish"] {
        let peer = peer::Peer::new().await;
        let pause = peer.pause(
            if phase == "before_start" {
                "/api/v1/freebuff/session/admission"
            } else {
                "/api/v1/chat/completions"
            },
            1,
        );
        let host = Box::pin(Host::new(&peer)).await;
        host.seed_device(&peer).await;
        let grant = host.grant().await;
        let task = host.public(request());
        let pending = host.pending(&grant, &task).await;
        assert_eq!(host.decision(&grant, &pending, "approve").await, 200);
        pause.reached().await;
        let pool = &host.state.local_runtime.as_ref().unwrap().pool;
        sqlx::query(if phase == "before_start" {
            "UPDATE local_freebuff_session_claims SET sealed='changed'"
        } else {
            "UPDATE local_freebuff_runs SET sealed='changed'"
        })
        .execute(pool)
        .await
        .unwrap();
        pause.resume();
        let (status, body) = task.await.unwrap();
        assert_eq!(status, 503, "{body}");
        assert_eq!(
            body["error"]["code"],
            "freebuff_business_run_journal_unavailable"
        );
        assert!(peer.finishes().is_empty());
        assert_eq!(
            peer.calls("/api/v1/agent-runs"),
            usize::from(phase == "before_finish")
        );
        assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 1);
        assert_eq!(claims(&host).await, 1);
        settled(&host, "failed", 10).await;
        host.close().await;
        peer.server.close().await;
    }
}
