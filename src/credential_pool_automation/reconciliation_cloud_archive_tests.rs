//! Real reconciliation, WebDAV read-back and durable local route-CAS races.
use super::*;
use crate::routing::config::RouteConfigSnapshot;
use crate::state::AppState;
use std::sync::Arc;
use std::time::Duration;

#[path = "reconciliation_archive_fixture.rs"]
mod fixture;
use fixture::ArchiveServer;

#[derive(Clone, Copy)]
enum CompetingWriter {
    None,
    SameRuntime,
    IndependentRuntime,
}

#[tokio::test]
async fn cloud_archive_revision_change_retains_source_credentials() {
    run_archive_case(CompetingWriter::SameRuntime).await;
}

#[tokio::test]
async fn cloud_archive_stale_runtime_cas_cannot_overwrite_new_routes() {
    run_archive_case(CompetingWriter::IndependentRuntime).await;
}

#[tokio::test]
async fn cloud_archive_verified_readback_allows_durable_prune() {
    run_archive_case(CompetingWriter::None).await;
}

async fn run_archive_case(writer: CompetingWriter) {
    let server = ArchiveServer::start().await;
    let root =
        std::env::temp_dir().join(format!("gateway-local-cloud-reconcile-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let routes = root.join("routes.yaml");
    fs::write(&routes, serde_json::to_vec(&serde_json::json!({
        "providers": [{"id":"p", "base_url":"https://example.invalid",
            "credential_archive_connection":{"type":"webdav", "endpoint":format!("{}/dav", server.endpoint),
                "directory":"pool", "allow_insecure_http":true},
            "credentials":[{"id":"a","api_key":"fixture-a"},{"id":"b","api_key":"fixture-b"}]}],
        "model_routes":[], "aliases":{}
    })).unwrap()).unwrap();
    let console = ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(root.join("state")),
        routes_file: Some(routes),
        ..Default::default()
    })
    .unwrap();
    let state = crate::runtime::build_app_state(local_config(console.clone()))
        .await
        .unwrap();
    let result = exercise_race(&state, &console, &server, writer).await;
    state.local_runtime.as_ref().unwrap().close().await;
    drop(state);
    drop(server);
    // Reload from disk, so an in-memory-only winner cannot satisfy the assertions.
    let reloaded = RouteConfigRuntime::new_local(&console).unwrap();
    let persisted = reloaded.route_config().snapshot();
    let persisted_result = verify_winner(&persisted, writer);
    drop(reloaded);
    crate::local_runtime::test_support::remove_test_root(&root).await;
    result.unwrap();
    persisted_result.unwrap();
}

async fn exercise_race(
    state: &Arc<AppState>,
    console: &ConsoleConfig,
    server: &ArchiveServer,
    writer: CompetingWriter,
) -> anyhow::Result<()> {
    let original = state.route_config.snapshot();
    let provider = original.document().providers[0].clone();
    let driver = CredentialAutomationDriver {
        id: "fixture-driver".into(),
        provider_ids: vec!["p".into()],
        timeout_secs: Some(5),
        transport: CredentialAutomationDriverTransport::Http {
            endpoint: format!("{}/driver", server.endpoint),
            secret_env: None,
        },
    };
    let reconcile = reconciliation::reconcile_provider_with_driver(
        state,
        &original,
        provider,
        &driver,
        CredentialPoolAutomationAction::Prune,
    );
    tokio::pin!(reconcile);
    // Poll the real reconciliation concurrently with the barrier; do not use sleeps
    // or leave a detached task that could mutate the routes after a failed assertion.
    tokio::select! {
        result = &mut reconcile => anyhow::bail!("reconciliation finished before archive read-back: {result:?}"),
        waited = tokio::time::timeout(Duration::from_secs(5), server.wait_for_readback()) => waited?,
    }
    server.verify_archive()?;
    anyhow::ensure!(state.route_config.snapshot().revision().id() == original.revision().id());
    anyhow::ensure!(
        state.route_config.snapshot().document().providers[0]
            .credentials
            .len()
            == 2
    );
    if !matches!(writer, CompetingWriter::None) {
        let mut repaired = original.document().clone();
        repaired.providers[0].label = Some("newer operator edit".into());
        repaired.providers[0].credentials[0].api_key = Some("fixture-repaired-a".into());
        let independent;
        let runtime = match writer {
            CompetingWriter::IndependentRuntime => {
                independent = RouteConfigRuntime::new_local(console)?;
                &independent
            }
            _ => state.route_config_runtime.as_ref().unwrap(),
        };
        runtime
            .commit_document(original.revision().id(), repaired, None)
            .await?;
        if matches!(writer, CompetingWriter::IndependentRuntime) {
            // The old instance's early revision guard still passes. Only durable CAS
            // can prevent the subsequent successful archive from overwriting this edit.
            anyhow::ensure!(
                state.route_config.snapshot().revision().id() == original.revision().id()
            );
        }
    }
    server.release_readback();
    let outcome = tokio::time::timeout(Duration::from_secs(5), &mut reconcile).await?;
    match writer {
        CompetingWriter::None => {
            let outcome = outcome?;
            anyhow::ensure!(outcome.created_count == 0 && outcome.pruned_count == 1);
            verify_winner(&state.route_config.snapshot(), writer)?;
        }
        CompetingWriter::SameRuntime => {
            let error = outcome
                .err()
                .ok_or_else(|| anyhow::anyhow!("stale prune succeeded"))?;
            anyhow::ensure!(error
                .to_string()
                .contains("Route revision changed during archival"));
            verify_winner(&state.route_config.snapshot(), writer)?;
        }
        CompetingWriter::IndependentRuntime => {
            let error = outcome
                .err()
                .ok_or_else(|| anyhow::anyhow!("stale CAS succeeded"))?;
            anyhow::ensure!(error.to_string().contains("active revision conflict"));
            anyhow::ensure!(
                state.route_config.snapshot().revision().id() == original.revision().id()
            );
            anyhow::ensure!(
                state.route_config.snapshot().document().providers[0]
                    .credentials
                    .len()
                    == 2
            );
        }
    }
    server.verify_archive()?;
    Ok(())
}

fn verify_winner(snapshot: &RouteConfigSnapshot, writer: CompetingWriter) -> anyhow::Result<()> {
    let provider = &snapshot.document().providers[0];
    let ids: Vec<_> = provider
        .credentials
        .iter()
        .filter_map(|credential| credential.id.as_deref())
        .collect();
    anyhow::ensure!(
        provider
            .credentials
            .last()
            .and_then(|credential| credential.api_key.as_deref())
            == Some("fixture-b")
    );
    if matches!(writer, CompetingWriter::None) {
        anyhow::ensure!(ids == ["b"]);
    } else {
        anyhow::ensure!(
            ids == ["a", "b"],
            "a stale archive prune removed source credentials"
        );
        anyhow::ensure!(provider.label.as_deref() == Some("newer operator edit"));
        anyhow::ensure!(provider.credentials[0].api_key.as_deref() == Some("fixture-repaired-a"));
    }
    Ok(())
}
