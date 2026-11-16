//! Mutable health may change without a route revision while inspect/START awaits the network.
use super::*;
pub(super) async fn attempts(host: &Host, count: usize) -> Vec<u32> {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let rows = host
                .state
                .local_runtime
                .as_ref()
                .unwrap()
                .list_audits(&crate::db::RequestAuditFilters::default())
                .await
                .unwrap();
            if rows.len() == count && rows.iter().all(|r| r.status != "running") {
                // Local audit persists this counter in route_trace, not as a View field.
                let mut result: Vec<_> = rows
                    .iter()
                    .map(|r| {
                        u32::try_from(
                            r.route_trace.as_ref().unwrap()["routeAttemptCount"]
                                .as_u64()
                                .unwrap(),
                        )
                        .unwrap()
                    })
                    .collect();
                result.sort();
                break result;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
async fn block_model(host: &Host) {
    let candidate = host
        .state
        .route_config
        .snapshot()
        .resolve_candidates(Some("open"))
        .remove(0);
    let blocked = host
        .state
        .local_runtime
        .as_ref()
        .unwrap()
        .model_failure(crate::db::RecordCredentialModelFailureInput {
            provider_account_id: candidate.provider_account_id,
            provider_credential_id: candidate.provider_credential_id,
            provider_credential_ref: candidate.payload.credential_id,
            protocol_profile: Some(candidate.protocol_profile),
            model: "open".into(),
            upstream_status: Some(401),
            upstream_retry_after_ms: None,
            error_message: Some("synthetic invalid credential".into()),
            classification: crate::provider_failure::classify_provider_failure(
                Some(401),
                None,
                Some("invalid credential"),
            ),
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(blocked.status, "blocked");
}
#[tokio::test]
async fn freebuff_public_chat_changed_quote_does_not_consume_an_outbound_attempt() {
    let peer = peer::Peer::new().await;
    *peer.mode.lock().unwrap() = "price_changed".into();
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let grant = host.grant().await;
    let task = host.public(request());
    let view = host.pending(&grant, &task).await;
    assert_eq!(host.decision(&grant, &view, "approve").await, 200);
    let (status, body) = task.await.unwrap();
    assert_eq!(status, 409, "{body}");
    assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 0);
    assert_eq!(claims(&host).await, 0);
    settled(&host, "failed", 10).await;
    assert_eq!(attempts(&host, 1).await, [0]);
    host.close().await;
    peer.server.close().await;
}
#[tokio::test]
async fn freebuff_public_chat_rechecks_health_after_inspect_and_start() {
    for after_start in [false, true] {
        let peer = peer::Peer::new().await;
        let pause = if after_start {
            peer.pause("/api/v1/agent-runs", 1)
        } else {
            peer.pause("/api/v1/freebuff/session", 2)
        };
        let host = Box::pin(Host::new(&peer)).await;
        host.seed_device(&peer).await;
        let revision = host
            .state
            .route_config
            .snapshot()
            .revision()
            .id()
            .to_owned();
        let grant = host.grant().await;
        let task = host.public(request());
        let view = host.pending(&grant, &task).await;
        assert_eq!(host.decision(&grant, &view, "approve").await, 200);
        pause.reached().await;
        block_model(&host).await;
        assert_eq!(host.state.route_config.snapshot().revision().id(), revision);
        pause.resume();
        let (status, body) = task.await.unwrap();
        assert_eq!(status, 503, "{body}");
        assert_eq!(body["error"]["code"], "upstream_credential_unavailable");
        assert_eq!(
            peer.calls("/api/v1/freebuff/session/admission"),
            usize::from(after_start)
        );
        assert_eq!(peer.calls("/api/v1/chat/completions"), 0);
        assert_eq!(
            claims(&host).await,
            1,
            "a committed unsent/paid claim must be retained"
        );
        if after_start {
            let finishes = peer.finishes();
            assert_eq!(finishes.len(), 1);
            assert_eq!(finishes[0]["status"], "cancelled");
            assert_eq!(finishes[0]["totalSteps"], 0);
            assert_eq!(finishes[0]["totalCredits"], 0);
        } else {
            assert!(peer.finishes().is_empty());
        }
        settled(&host, "failed", 10).await;
        assert_eq!(attempts(&host, 1).await, [u32::from(after_start)]);
        host.close().await;
        peer.server.close().await;
    }
}
