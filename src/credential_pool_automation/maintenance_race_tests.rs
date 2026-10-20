//! Explicit barriers prove stale previews cannot remove a repaired credential.
use super::*;
use std::time::Duration;

#[tokio::test]
async fn maintenance_archive_revision_change_retains_all_selected_sources() {
    archive_race(false).await;
}

#[tokio::test]
async fn maintenance_durable_cas_rejects_an_independent_repair_without_false_success() {
    archive_race(true).await;
}

async fn archive_race(independent: bool) {
    let h = Fixture::start(true, false).await;
    let preview = h.preview().await;
    h.driver.pause_archive.store(true, Ordering::SeqCst);
    {
        let apply = h.request("apply", selection(&preview, json!(["a"])));
        tokio::pin!(apply);
        tokio::select! {
            response = &mut apply => panic!("apply finished before readback: {}", response.status()),
            waited = tokio::time::timeout(Duration::from_secs(5), h.driver.readback.notified()) => waited.unwrap(),
        }
        let snapshot = h.state.route_config.snapshot();
        let mut document = snapshot.document().clone();
        document.providers[0].credentials[0].api_key = Some("fixture-repaired-a".into());
        let other;
        let runtime = if independent {
            other = RouteConfigRuntime::new_local(&h.console).unwrap();
            &other
        } else {
            h.state.route_config_runtime.as_ref().unwrap()
        };
        runtime
            .commit_document(snapshot.revision().id(), document, None)
            .await
            .unwrap();
        h.driver.release_archive.notify_one();
        let response = tokio::time::timeout(Duration::from_secs(5), &mut apply)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let receipt = decode(response).await;
        if independent {
            assert_eq!(receipt["receipt"]["outcome"], "commit_unconfirmed");
            assert!(receipt["receipt"]["removedCount"].is_null());
            assert_eq!(
                receipt["receipt"]["items"][0]["status"],
                "commit_unconfirmed"
            );
        } else {
            assert_eq!(receipt["receipt"]["outcome"], "revision_changed");
            assert_eq!(receipt["receipt"]["removedCount"], 0);
            assert_eq!(
                receipt["receipt"]["items"][0]["status"],
                "retained_revision_changed"
            );
        }
    }
    assert_eq!(h.driver.archives.lock().unwrap().len(), 1);
    let persisted = h.close().await;
    assert_eq!(persisted.providers[0].credentials.len(), 2);
    assert_eq!(
        persisted.providers[0].credentials[0].api_key.as_deref(),
        Some("fixture-repaired-a")
    );
}

#[tokio::test]
async fn maintenance_route_change_during_reinspection_retains_repaired_source() {
    let h = Fixture::start(false, false).await;
    let preview = h.preview().await;
    h.driver.pause.store(true, Ordering::SeqCst);
    {
        let apply = h.request("apply", selection(&preview, json!(["a"])));
        tokio::pin!(apply);
        tokio::select! {
            response = &mut apply => panic!("apply completed before driver barrier: {}", response.status()),
            waited = tokio::time::timeout(Duration::from_secs(5), h.driver.entered.notified()) => waited.unwrap(),
        }
        assert_eq!(
            h.request("preview", json!({})).await.status(),
            StatusCode::CONFLICT
        );
        let snapshot = h.state.route_config.snapshot();
        let mut document = snapshot.document().clone();
        document.providers[0].credentials[0].api_key = Some("fixture-repaired-a".into());
        h.state
            .route_config_runtime
            .as_ref()
            .unwrap()
            .commit_document(snapshot.revision().id(), document, None)
            .await
            .unwrap();
        h.driver.release.notify_one();
        assert_eq!(apply.await.status(), StatusCode::CONFLICT);
    }
    assert!(!h.root.join("archive").exists());
    let persisted = h.close().await;
    assert_eq!(persisted.providers[0].credentials.len(), 2);
    assert_eq!(
        persisted.providers[0].credentials[0].api_key.as_deref(),
        Some("fixture-repaired-a")
    );
}
