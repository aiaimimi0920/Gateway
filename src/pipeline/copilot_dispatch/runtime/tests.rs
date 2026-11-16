use super::*;
use crate::pipeline::stage_rate_limit::provider_attempt_gate;
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
#[path = "../../copilot_identity/fixture.rs"]
mod fixture;
#[path = "startup_tests.rs"]
mod startup_tests;

fn settings(name: &str) -> Settings {
    let root = PathBuf::from(std::env::var_os("GATEWAY_PROBE_ROOT").expect("isolated probe root"))
        .join(name);
    std::fs::create_dir_all(&root).unwrap();
    Settings {
        executable: PathBuf::from(
            std::env::var_os("GATEWAY_SYNTHETIC_COPILOT").expect("fixture EXE"),
        ),
        workspace_root: root,
    }
}

async fn fixture(prompt: &str) -> fixture::Fixture {
    let mut f = fixture::Fixture::new().await;
    f.replace(|doc| {
        doc.providers[0].credentials[0].copilot_github_token = Some("synthetic-token".into())
    });
    f.ctx.cash_route_snapshot = Some(f.console.state.route_config.snapshot());
    f.ctx.canonical_req = crate::protocol::openai::normalize_chat_completions(json!({
        "model":"public-alias", "messages":[{"role":"user","content":prompt}]
    }))
    .unwrap();
    f
}

#[tokio::test]
#[ignore = "requires isolated GATEWAY_PROBE_ROOT and GATEWAY_SYNTHETIC_COPILOT fixture EXE"]
async fn copilot_dispatch_success_keeps_usage_unknown_and_cleans_confirmed_child() {
    let f = fixture("known-usage").await;
    let identity =
        CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "synthetic-model").unwrap();
    let gate = provider_attempt_gate(&f.ctx, &f.console.state, "copilot").unwrap();
    let slot = Semaphore::new(1);
    let settings = settings("success");
    let root = settings.workspace_root.clone();
    let result = gate
        .execute_observed(
            || run(&identity, &gate, settings.clone(), &slot),
            |_| {},
            &crate::retry::RetryPolicy {
                max_retries: 0,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(result.text, "synthetic answer");
    assert!(result.usage.is_none());
    assert!(result.upstream_status.is_none());
    assert!(result.finish_reason.is_none());
    assert!(result.model_declaration.is_none());
    assert!(result.model.is_empty());
    assert_eq!(slot.available_permits(), 1);
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    assert_eq!(
        f.ctx
            .route_attempt_count
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_eq!(
        f.console
            .state
            .route_config
            .snapshot()
            .resolve_candidates(Some("public-alias"))
            .len(),
        1
    );
    drop(identity);
    f.close().await;
}

#[tokio::test]
#[ignore = "requires isolated GATEWAY_PROBE_ROOT and GATEWAY_SYNTHETIC_COPILOT fixture EXE"]
async fn copilot_dispatch_sdk_error_is_safe_terminal_and_releases_confirmed_slot() {
    let f = fixture("denied").await;
    let identity =
        CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "synthetic-model").unwrap();
    let gate = provider_attempt_gate(&f.ctx, &f.console.state, "copilot").unwrap();
    let slot = Semaphore::new(1);
    let settings = settings("denied");
    let root = settings.workspace_root.clone();
    let error = gate
        .execute_observed(
            || run(&identity, &gate, settings.clone(), &slot),
            |_| {},
            &crate::retry::RetryPolicy {
                max_retries: 3,
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert!(!error.retryable);
    assert!(error.native_transport.is_none());
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    assert!(!format!("{error:?}").contains("synthetic-token"));
    assert_eq!(
        f.ctx
            .route_attempt_count
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_eq!(slot.available_permits(), 1);
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    drop(identity);
    f.close().await;
}

#[tokio::test]
#[ignore = "requires isolated GATEWAY_PROBE_ROOT and GATEWAY_SYNTHETIC_COPILOT fixture EXE"]
async fn copilot_dispatch_preflight_failure_does_not_quarantine_slot_or_start_child() {
    let f = fixture("ok").await;
    let identity =
        CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "synthetic-model").unwrap();
    let gate = provider_attempt_gate(&f.ctx, &f.console.state, "copilot").unwrap();
    let slot = Semaphore::new(1);
    let settings = settings("revoked-before-start");
    let root = settings.workspace_root.clone();
    f.replace(|doc| doc.providers[0].credentials[0].enabled = Some(false));
    assert!(run(&identity, &gate, settings, &slot).await.is_err());
    assert_eq!(slot.available_permits(), 1);
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    drop(identity);
    f.close().await;
}

#[tokio::test]
#[ignore = "requires isolated GATEWAY_PROBE_ROOT and GATEWAY_SYNTHETIC_COPILOT fixture EXE"]
async fn copilot_dispatch_drop_during_generation_quarantines_slot_and_preserves_directory() {
    let f = fixture("hang").await;
    let identity =
        CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "synthetic-model").unwrap();
    let gate = provider_attempt_gate(&f.ctx, &f.console.state, "copilot").unwrap();
    let slot = Semaphore::new(1);
    let settings = settings("cancel");
    let root = settings.workspace_root.clone();
    let mut execution = Box::pin(run(&identity, &gate, settings.clone(), &slot));
    tokio::select! {
        result = &mut execution => panic!("unexpected completion: {result:?}"),
        _ = async {
            tokio::time::timeout(Duration::from_secs(15), async {
                loop {
                    let sent = std::fs::read_dir(&root).unwrap().filter_map(Result::ok).any(|entry| {
                        std::fs::read_to_string(entry.path().join("rpc.jsonl"))
                            .is_ok_and(|log| log.contains("session.sendAndWait"))
                    });
                    if sent { break; }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }).await.unwrap();
        } => {}
    }
    drop(execution);
    drop(identity);
    assert_eq!(slot.available_permits(), 0);
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 1);
    assert!(run(&f.bind(), &gate, settings, &slot).await.is_err());
    f.close().await;
}

#[test]
fn copilot_dispatch_lease_is_armed_before_start_and_released_only_when_confirmed() {
    for unconfirmed in [false, true] {
        let slot = Semaphore::new(1);
        drop(Lease {
            permit: Some(slot.try_acquire().unwrap()),
            unconfirmed,
        });
        assert_eq!(slot.available_permits(), usize::from(!unconfirmed));
    }
}
