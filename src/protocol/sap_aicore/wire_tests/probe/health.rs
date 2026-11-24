//! Probe audits retain send-phase facts even when OAuth or body collection is cancelled.
use super::*;

#[tokio::test]
async fn sap_aicore_probe_classified_errors_and_unsent_auth_are_distinct() {
    Box::pin(async {
        for (oauth, status) in [
            (true, 401),
            (true, 429),
            (false, 400),
            (false, 401),
            (false, 403),
            (false, 429),
            (false, 503),
        ] {
            let peer =
                peer::Peer::new("chat", false, false, if oauth { 200 } else { status }).await;
            *peer.retry_after.lock().unwrap() = Some("120".into());
            if oauth {
                *peer.token.lock().unwrap() = (status, b"private-token-error".to_vec());
            }
            let f = Fixture::new(&peer).await;
            let report =
                super::super::super::test_support::origin(peer.origin.clone(), call(&f)).await;
            assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
            let rows = f.audits(1).await;
            let audit = &rows[0];
            assert_eq!(
                audit.route_trace.as_ref().unwrap()["routeAttemptCount"],
                if oauth { 0 } else { 1 }
            );
            assert_eq!(
                audit.upstream_status,
                if oauth { None } else { Some(i32::from(status)) }
            );
            assert_eq!(audit.provider_account_id.is_some(), !oauth);
            let evidence = &audit.route_trace.as_ref().unwrap()["sapProbe"];
            assert_eq!(evidence["attempts"], if oauth { 0 } else { 1 });
            assert_eq!(
                evidence["failure"]["phase"],
                if oauth { "oauth" } else { "inference" }
            );
            assert_eq!(evidence["failure"]["status"], status);
            assert_eq!(evidence["failure"]["retryAfterMs"], 120_000);
            assert!(!serde_json::to_string(&rows)
                .unwrap()
                .contains("private-token-error"));
            let states = f
                .state
                .local_runtime
                .as_ref()
                .unwrap()
                .list_model_states(db::CredentialModelStateFilters::default())
                .await
                .unwrap();
            if !oauth && matches!(status, 429 | 503) {
                assert_eq!(states.len(), 1);
                assert_eq!(states[0].provider_credential_ref.as_deref(), Some("exact"));
                assert_eq!(states[0].failure_count, 1);
                assert_eq!(states[0].last_upstream_status, Some(i32::from(status)));
                if status == 429 {
                    let until = time::OffsetDateTime::parse(
                        states[0].cooldown_until.as_deref().unwrap(),
                        &time::format_description::well_known::Rfc3339,
                    )
                    .unwrap();
                    assert!(until - time::OffsetDateTime::now_utc() > time::Duration::seconds(100));
                }
            } else {
                assert!(states.is_empty(), "{states:?}");
            }
            assert_eq!(peer.calls.lock().unwrap().len(), usize::from(!oauth));
            f.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_probe_oauth_cancel_has_zero_actual_identity() {
    Box::pin(async {
        let peer = peer::Peer::new("chat", false, false, 200).await;
        peer.oauth_hold
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let f = Fixture::new(&peer).await;
        let mut pending = Box::pin(super::super::super::test_support::origin(
            peer.origin.clone(),
            call(&f),
        ));
        tokio::select! {
            _ = &mut pending => panic!("OAuth should wait"),
            arrived = tokio::time::timeout(Duration::from_secs(3), async {
                while peer.oauth.lock().unwrap().is_empty() { tokio::task::yield_now().await; }
            }) => { arrived.expect("OAuth must actually start before cancellation"); },
        }
        assert_eq!(peer.oauth.lock().unwrap().len(), 1);
        drop(pending);
        let rows = f.audits(1).await;
        assert_eq!(rows[0].status, "cancelled");
        assert!(rows[0].provider_account_id.is_none());
        assert!(rows[0].upstream_status.is_none());
        assert_eq!(
            rows[0].route_trace.as_ref().unwrap()["routeAttemptCount"],
            0
        );
        assert!(rows[0].route_trace.as_ref().unwrap()["realCredentialRef"].is_null());
        assert!(peer.calls.lock().unwrap().is_empty());
        f.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_probe_bad_json_preserves_received_http_status() {
    Box::pin(async {
        let peer = peer::Peer::raw(b"{bad-json".to_vec()).await;
        let f = Fixture::new(&peer).await;
        let report = super::super::super::test_support::origin(peer.origin.clone(), call(&f)).await;
        assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
        let rows = f.audits(1).await;
        assert_eq!(rows[0].upstream_status, Some(200));
        assert_eq!(
            rows[0].route_trace.as_ref().unwrap()["routeAttemptCount"],
            1
        );
        assert_eq!(peer.calls.lock().unwrap().len(), 1);
        f.finish().await;
        peer.stop().await;
    })
    .await;
}
