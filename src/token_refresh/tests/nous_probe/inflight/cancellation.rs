//! Cancellation owns the old body and audit, never the newly published credential.
use super::*;

#[tokio::test]
async fn nous_probe_inflight_cancel_after_rotation_releases_body_without_replay() {
    Box::pin(async {
        let f = fixture().await;
        let pinned = f.runtime.route_config().snapshot();
        let old = pinned.select_credential_probe_target("account").unwrap();
        let original = request(&f);
        let peer = DelayedPeer::new(200).await;
        let (console, local) = state(&f, peer.client()).await;
        let mock = Mock::new(
            vec![(
                200,
                crate::token_refresh::tests::nous_target::subscription_reply(),
            )],
            0,
        )
        .await;
        let mut pending = Box::pin(crate::protocol::nous_portal::test_support::origin(
            peer.origin.clone(),
            Box::pin(call(&console.state, &old)),
        ));
        let rotate = async {
            tokio::time::timeout(Duration::from_secs(5), peer.reply.arrived.acquire())
                .await
                .unwrap()
                .unwrap()
                .forget();
            force_due(&f);
            Box::pin(refresh(&f, &mock, &mut Pending::new())).await;
            assert_eq!(
                load(&f, &original).phase(),
                crate::console::refresh_grants::GrantPhase::Published
            );
        };
        tokio::select! {
            _ = &mut pending => panic!("old request must wait for the held body"),
            _ = Box::pin(rotate) => {}
        }
        // No release permit: only dropping the caller can end the held body.
        drop(pending);
        tokio::time::timeout(Duration::from_secs(5), peer.reply.closed.acquire())
            .await
            .unwrap()
            .unwrap()
            .forget();
        let rows = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let rows = local
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                if rows.len() == 1 && rows[0].status == "cancelled" {
                    break rows;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(rows[0].total_tokens.is_none());
        assert!(rows[0].upstream_status.is_none());
        assert_eq!(peer.reply.calls.lock().unwrap().len(), 1);
        assert_eq!(mock.count(), 1);
        assert!(local
            .list_model_states(db::CredentialModelStateFilters::default())
            .await
            .unwrap()
            .is_empty());
        // Cancellation must not undo durable publication or retire the successor grant.
        assert_eq!(
            load(&f, &original).phase(),
            crate::console::refresh_grants::GrantPhase::Published
        );
        let fresh = pinned.select_credential_probe_target("account").unwrap();
        assert_ne!(old.payload.api_key, fresh.payload.api_key);
        assert_eq!(
            fresh.payload.base_url,
            crate::protocol::nous_portal::InferenceTarget::Subscription.base_url()
        );
        let current = crate::protocol::nous_portal::test_support::origin(
            peer.origin.clone(),
            Box::pin(call(&console.state, &fresh)),
        )
        .await;
        assert_eq!(
            current.status,
            ProviderPayloadProbeStatus::Passed,
            "{current:?}"
        );
        assert_eq!(current.health_recovery, Some(ModelHealthRecovery::Applied));
        let calls = peer.reply.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 2);
        for (headers, target) in calls.iter().zip([&old, &fresh]) {
            assert_eq!(
                headers["authorization"],
                format!("Bearer {}", target.payload.api_key)
            );
        }
        assert_eq!(mock.count(), 1);
        let rows = local
            .list_audits(&db::RequestAuditFilters::default())
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.iter().filter(|r| r.status == "cancelled").count(), 1);
        assert_eq!(
            rows.iter()
                .filter(|r| r.status == "completed" && r.total_tokens == Some(5))
                .count(),
            1
        );
        let saved = serde_json::to_string(&rows).unwrap();
        for secret in [
            &old.payload.api_key,
            &fresh.payload.api_key,
            &original.material.refresh,
        ] {
            assert!(!saved.contains(secret));
        }
        drop(console);
        local.close().await;
        mock.stop().await;
    })
    .await;
}
