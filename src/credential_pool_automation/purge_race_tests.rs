//! Real runtime CAS and loopback cloud I/O; deliberately bypass admission to test fencing.
use super::*;
use crate::credential_pool_automation::archive_purge::execute_with_barrier;
use crate::credential_pool_storage::{archive::preserve, purge::ArchivePurgePlan};
use crate::{routing::config::RouteConfigSnapshot, state::AppState};
use std::{collections::HashSet, sync::Arc, time::Duration};
#[path = "purge_race_fixture.rs"]
mod fixture;
use fixture::PurgeServer;

#[derive(Clone, Copy)]
enum Case {
    InFlight,
    StaleReplica,
    LateDelete,
    CompetingPurges,
    PartialFailure,
}
#[tokio::test]
async fn purge_fences_an_inflight_archive_before_source_removal() {
    run(Case::InFlight).await;
}
#[tokio::test]
async fn stale_replica_purge_cannot_delete_after_authority_changes() {
    run(Case::StaleReplica).await;
}
#[tokio::test]
async fn late_purge_delete_cannot_remove_new_revision_recovery_copy() {
    run(Case::LateDelete).await;
}
#[tokio::test]
async fn stale_local_purge_snapshot_cannot_delete_after_another_barrier() {
    run(Case::CompetingPurges).await;
}
#[tokio::test]
async fn partial_purge_failure_keeps_the_committed_barrier() {
    run(Case::PartialFailure).await;
}

async fn run(case: Case) {
    let server = PurgeServer::start().await;
    let root = std::env::temp_dir().join(format!("gateway-local-purge-race-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let routes = root.join("routes.yaml");
    fs::write(&routes,serde_json::to_vec(&serde_json::json!({"providers":[{"id":"p","base_url":"https://example.invalid","credentials":[{"id":"a","api_key":"fixture-a"},{"id":"b","api_key":"fixture-b"}],"credential_archive_connection":{"type":"webdav","endpoint":server.endpoint,"directory":"pool","allow_insecure_http":true}}],"model_routes":[],"aliases":{}})).unwrap()).unwrap();
    let console = ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(root.join("state")),
        routes_file: Some(routes),
        ..Default::default()
    })
    .unwrap();
    let state = crate::runtime::build_app_state(local_config(console.clone()))
        .await
        .unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        exercise(&state, &console, &server, case),
    )
    .await;
    state.local_runtime.as_ref().unwrap().close().await;
    drop(state);
    drop(server);
    let reloaded = RouteConfigRuntime::new_local(&console).unwrap();
    let persisted = reloaded.route_config().snapshot();
    let credentials = &persisted.document().providers[0].credentials;
    let expected = if matches!(case, Case::LateDelete) {
        vec!["b"]
    } else {
        vec!["a", "b"]
    };
    let ids: Vec<_> = credentials.iter().filter_map(|c| c.id.as_deref()).collect();
    let preserved = ids == expected
        && (!matches!(case, Case::StaleReplica)
            || credentials[0].api_key.as_deref() == Some("repaired-fixture-a"));
    drop(reloaded);
    crate::local_runtime::test_support::remove_test_root(&root).await;
    result.unwrap().unwrap();
    assert!(
        preserved,
        "purge/source CAS lost a credential or repaired key"
    );
}
fn driver(server: &PurgeServer) -> CredentialAutomationDriver {
    CredentialAutomationDriver {
        id: "fixture".into(),
        provider_ids: vec!["p".into()],
        timeout_secs: Some(5),
        transport: CredentialAutomationDriverTransport::Http {
            endpoint: format!("{}/driver", server.endpoint),
            secret_env: None,
        },
    }
}
async fn reconcile(
    state: &Arc<AppState>,
    snapshot: &RouteConfigSnapshot,
    server: &PurgeServer,
) -> anyhow::Result<ReconcileOutcome> {
    reconciliation::reconcile_provider_with_driver(
        state,
        snapshot,
        snapshot.document().providers[0].clone(),
        &driver(server),
        CredentialPoolAutomationAction::Prune,
    )
    .await
}
async fn capture(snapshot: &RouteConfigSnapshot) -> anyhow::Result<ArchivePurgePlan> {
    ArchivePurgePlan::capture_fixture(&snapshot.document().providers[0]).await
}
async fn exercise(
    state: &Arc<AppState>,
    console: &ConsoleConfig,
    server: &PurgeServer,
    case: Case,
) -> anyhow::Result<()> {
    let original = state.route_config.snapshot();
    if matches!(case, Case::InFlight) {
        server.pause_readback();
        let prune = reconcile(state, &original, server);
        tokio::pin!(prune);
        tokio::select! { outcome=&mut prune=>anyhow::bail!("prune finished before readback: {outcome:?}"), _=server.wait_readback()=>{} }
        let plan = capture(&original).await?;
        anyhow::ensure!(execute_with_barrier(state, &original, "p", plan).await? == 1);
        anyhow::ensure!(server.records().is_empty());
        server.release_readback();
        anyhow::ensure!(prune.await.is_err());
        anyhow::ensure!(
            state.route_config.snapshot().document().providers[0]
                .credentials
                .len()
                == 2
        );
        return Ok(());
    }
    let ids = if matches!(case, Case::PartialFailure) {
        HashSet::from(["a".into(), "b".into()])
    } else {
        HashSet::from(["a".into()])
    };
    preserve(
        &state.config,
        &original.document().providers[0],
        &ids,
        original.revision().id(),
    )
    .await?;
    let plan = capture(&original).await?;
    if matches!(case, Case::StaleReplica) {
        let independent = RouteConfigRuntime::new_local(console)?;
        let mut document = original.document().clone();
        document.providers[0].credentials[0].api_key = Some("repaired-fixture-a".into());
        independent
            .commit_document(original.revision().id(), document, None)
            .await?;
        anyhow::ensure!(state.route_config.snapshot().revision().id() == original.revision().id());
        anyhow::ensure!(execute_with_barrier(state, &original, "p", plan)
            .await
            .is_err());
        anyhow::ensure!(server.delete_count() == 0 && server.records().len() == 1);
        return Ok(());
    }
    if matches!(case, Case::PartialFailure) {
        server.fail_second_delete();
        let error = execute_with_barrier(state, &original, "p", plan)
            .await
            .unwrap_err();
        anyhow::ensure!(error.to_string().contains("1 confirmed deletions"));
        anyhow::ensure!(
            state.route_config.snapshot().revision().sequence()
                == original.revision().sequence() + 1
        );
        anyhow::ensure!(server.records().len() == 1);
        return Ok(());
    }
    let competitor = if matches!(case, Case::CompetingPurges) {
        Some(capture(&original).await?)
    } else {
        None
    };
    server.pause_delete();
    let purge = execute_with_barrier(state, &original, "p", plan);
    tokio::pin!(purge);
    tokio::select! { result=&mut purge=>anyhow::bail!("purge finished before DELETE barrier: {result:?}"), _=server.wait_delete()=>{} }
    let barrier = state.route_config.snapshot();
    anyhow::ensure!(barrier.revision().sequence() == original.revision().sequence() + 1);
    if let Some(competitor) = competitor {
        anyhow::ensure!(execute_with_barrier(state, &original, "p", competitor)
            .await
            .is_err());
        anyhow::ensure!(server.delete_count() == 1);
    } else {
        anyhow::ensure!(reconcile(state, &barrier, server).await?.pruned_count == 1);
        let records = server.records();
        anyhow::ensure!(records.len() == 2);
        anyhow::ensure!(records
            .iter()
            .any(|r| r["sourceRevision"] == barrier.revision().id()));
    }
    server.release_delete();
    anyhow::ensure!(purge.await? == 1);
    let records = server.records();
    if matches!(case, Case::LateDelete) {
        anyhow::ensure!(
            records.len() == 1 && records[0]["sourceRevision"] == barrier.revision().id()
        );
    } else {
        anyhow::ensure!(records.is_empty());
    }
    Ok(())
}
