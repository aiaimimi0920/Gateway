//! Durable refresh publication must invalidate captured probe payloads before inference sends.
use super::{nous_fixture::*, nous_scheduler_fixture::*, *};
use crate::{
    db,
    provider_runtime::{
        probe_console_target_with_request, ConsoleProbeRequest, ProviderPayloadProbeStatus,
    },
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
// This fixture only admits one nonstreaming text reply, not public tool-protocol flows.
mod native {
    use super::*;
    pub(super) fn reply(family: &str, tools: bool, stream: bool, terminal: bool) -> Vec<u8> {
        assert_eq!(family, "chat");
        assert!(!tools && !stream && terminal);
        json!({"id":"probe-reply","model":"test-model","choices":[{"index":0,
            "message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}})
        .to_string()
        .into_bytes()
    }
}
mod inflight;
#[path = "../../protocol/dim_oauth/pipeline_tests/peer.rs"]
mod peer;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;

async fn state(
    f: &LocalFixture,
    client: rquest::Client,
) -> (
    support::ConsoleStateFixture,
    crate::local_runtime::LocalRuntime,
) {
    let mut console = support::ConsoleStateFixture::new(
        f.runtime.route_config().snapshot().document().clone(),
        true,
    );
    let local = crate::local_runtime::LocalRuntime::open(&f.root.join("probe-audits"))
        .await
        .unwrap();
    let state = Arc::get_mut(&mut console.state).unwrap();
    state.route_config = f.runtime.route_config().clone();
    state.route_config_runtime = Some(Arc::new(f.runtime.clone()));
    state.local_runtime = Some(local.clone());
    state.upstream_client.http = client;
    (console, local)
}
async fn call(
    state: &AppState,
    target: &crate::routing::config::CredentialProbeTarget,
) -> crate::provider_runtime::ProviderPayloadProbeReport {
    let options: ConsoleProbeRequest =
        serde_json::from_value(json!({"model":"test-model","prompt":"only OK"})).unwrap();
    probe_console_target_with_request(state, target, Some(&options))
        .await
        .1
}
#[tokio::test]
async fn nous_probe_old_target_is_not_sent_after_durable_rotation_and_new_target_works() {
    Box::pin(async {
        let f = fixture().await;
        let pinned = f.runtime.route_config().snapshot();
        let old = pinned.select_credential_probe_target("account").unwrap();
        let original = request(&f);
        let peer = peer::Peer::new("chat", false, false, 200).await;
        let (console, local) = state(&f, peer.client()).await;
        let mock = Mock::new(vec![(200, super::nous_target::subscription_reply())], 0).await;
        force_due(&f);
        refresh(&f, &mock, &mut Pending::new()).await;
        assert_eq!(
            load(&f, &original).phase(),
            crate::console::refresh_grants::GrantPhase::Published
        );
        let fresh = pinned.select_credential_probe_target("account").unwrap();
        assert_eq!(
            fresh.payload.base_url,
            crate::protocol::nous_portal::InferenceTarget::Subscription.base_url()
        );
        assert_ne!(old.payload.api_key, fresh.payload.api_key);
        crate::protocol::nous_portal::test_support::origin(peer.origin.clone(), async {
            let stale = call(&console.state, &old).await;
            assert_eq!(stale.status, ProviderPayloadProbeStatus::Failed);
            assert!(stale.message.contains("call not sent"));
            assert!(peer.calls.lock().unwrap().is_empty());
            let current = call(&console.state, &fresh).await;
            assert_eq!(
                current.status,
                ProviderPayloadProbeStatus::Passed,
                "{current:?}"
            );
        })
        .await;
        let calls = peer.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "/v1/chat/completions");
        assert_eq!(
            calls[0].1["authorization"],
            format!("Bearer {}", fresh.payload.api_key)
        );
        assert_eq!(
            mock.count(),
            1,
            "the model probe must not consume another refresh token"
        );
        let rows = local
            .list_audits(&db::RequestAuditFilters::default())
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter()
                .filter(|r| r.status == "failed" && r.total_tokens.is_none())
                .count(),
            1
        );
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
        peer.stop().await;
        mock.stop().await;
    })
    .await;
}

#[tokio::test]
async fn nous_probe_captured_enabled_account_is_not_sent_after_disable() {
    Box::pin(async {
        let f = fixture().await;
        let snapshot = f.runtime.route_config().snapshot();
        let target = snapshot.select_credential_probe_target("account").unwrap();
        let peer = peer::Peer::new("chat", false, false, 200).await;
        let (console, local) = state(&f, peer.client()).await;
        let mut doc = snapshot.document().clone();
        doc.providers[0].credentials[0].enabled = Some(false);
        f.runtime
            .commit_document(snapshot.revision().id(), doc, None)
            .await
            .unwrap();
        let result = crate::protocol::nous_portal::test_support::origin(
            peer.origin.clone(),
            call(&console.state, &target),
        )
        .await;
        assert_eq!(result.status, ProviderPayloadProbeStatus::Failed);
        assert!(result.message.contains("call not sent"));
        assert!(peer.calls.lock().unwrap().is_empty());
        let rows = local
            .list_audits(&db::RequestAuditFilters::default())
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "failed");
        assert!(local
            .list_model_states(db::CredentialModelStateFilters::default())
            .await
            .unwrap()
            .is_empty());
        drop(console);
        local.close().await;
        peer.stop().await;
    })
    .await;
}
