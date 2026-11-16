//! Drop/reopen exposes uncertain wire facts; no detached finalizer or automatic recovery send.
use super::super::run_journal::view;
use super::*;
use crate::{
    local_runtime::LocalRuntime,
    protocol::freebuff::catalog::{BusinessOrigin, CatalogSession, ClientSurface},
};

#[tokio::test]
async fn freebuff_public_run_journal_drop_reopen_retains_each_uncertain_phase_without_replay() {
    for phase in ["start", "headers", "observed", "finish"] {
        let peer = peer::Peer::new().await;
        let pause = peer.pause(
            if matches!(phase, "start" | "finish") {
                "/api/v1/agent-runs"
            } else {
                "/api/v1/chat/completions"
            },
            if phase == "finish" { 2 } else { 1 },
        );
        if phase == "observed" {
            *peer.mode.lock().unwrap() = "stream_known".into();
        }
        let host = Box::pin(Host::new(&peer)).await;
        host.seed_device(&peer).await;
        let grant = host.grant().await;
        let observed = Arc::new(Notify::new());
        let (_cancel, task) = caller(&host, &peer, "caller", observed.clone());
        approve(&host, &task).await;
        if phase == "observed" {
            body_ready(&pause, &observed).await;
        } else {
            pause.reached().await;
        }
        let journal = view(&host, &grant).await;
        let run = &journal["claim"]["run"];
        assert_eq!(
            run["phase"],
            match phase {
                "start" => "start_pending",
                "headers" => "model_pending",
                "observed" => "model_observed",
                _ => "finish_pending",
            }
        );
        assert_eq!(run["runIdRecorded"], phase != "start");
        if phase == "headers" {
            assert!(run["totals"].is_null());
        } else {
            assert_eq!(
                run["totals"]["totalCredits"],
                if phase == "start" { 0 } else { 13 }
            );
        }
        task.abort();
        assert!(matches!(task.await, Err(error) if error.is_cancelled()));
        settled(&host, "cancelled", 10).await;
        if phase != "observed" {
            pause.resume();
        }
        let hits = peer.hits.lock().unwrap().len();
        let root = host.root.clone();
        host.stop().await;
        let local = LocalRuntime::open(&root.join("state")).await.unwrap();
        let session = CatalogSession::new(
            BusinessOrigin::Codebuff,
            "synthetic-account",
            ClientSurface::Cli,
        )
        .unwrap();
        let restored = local
            .freebuff_claim_journal("pool", "account", &session.stored_claim_scope(), None)
            .await
            .unwrap();
        let restored = serde_json::to_value(restored).unwrap();
        assert_eq!(restored["claim"], journal["claim"]);
        assert_eq!(
            peer.hits.lock().unwrap().len(),
            hits,
            "reopen is offline, never START/FINISH/end/rebuy"
        );
        assert_eq!(peer.finishes().len(), usize::from(phase == "finish"));
        local.close().await;
        peer.server.close().await;
        crate::local_runtime::test_support::remove_test_root(&root).await;
    }
}

#[tokio::test]
async fn freebuff_public_run_journal_unknown_start_and_rejected_finish_remain_distinct() {
    for mode in ["start_unknown", "finish_rejected", "unknown"] {
        let peer = peer::Peer::new().await;
        *peer.mode.lock().unwrap() = mode.into();
        let host = Box::pin(Host::new(&peer)).await;
        host.seed_device(&peer).await;
        let grant = host.grant().await;
        let task = host.public(request());
        let pending = host.pending(&grant, &task).await;
        assert_eq!(host.decision(&grant, &pending, "approve").await, 200);
        assert_eq!(task.await.unwrap().0, 502);
        let journal = view(&host, &grant).await;
        assert_eq!(
            journal["claim"]["run"]["phase"],
            match mode {
                "start_unknown" => "start_pending",
                "unknown" => "step_stopped",
                _ => "finish_pending",
            }
        );
        if mode == "unknown" {
            assert!(journal["claim"]["run"]["totals"].is_null());
        }
        assert!(journal["claim"]["run"]["finishHttpStatus"].is_null());
        assert_eq!(
            peer.finishes().len(),
            usize::from(mode == "finish_rejected")
        );
        host.close().await;
        peer.server.close().await;
    }
}
