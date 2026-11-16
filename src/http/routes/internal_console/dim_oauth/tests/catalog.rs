//! Real management reads are side-effect-free; only normal explicit revision/CAS commits publish.
use super::{fixture::*, *};
use crate::console::dim_oauth::SessionStatus;
use std::{sync::atomic::Ordering, time::Duration};
const PATH: &str = "/v1/internal/gateway/console/dim-model-catalog";
async fn saved() -> (support::ConsoleStateFixture, Peer, String, String) {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(3600, Duration::ZERO).await;
    let view = create(state, &grant, create_body(state), &peer).await;
    complete(state, &view, &grant, &peer).await;
    assert_eq!(
        action(state, &view, Some(&grant), "import", None, &peer)
            .await
            .status(),
        StatusCode::OK
    );
    wait_status(&session(state, &view), SessionStatus::Succeeded).await;
    let id = state.route_config.snapshot().document().providers[0].credentials[0]
        .id
        .clone()
        .unwrap();
    (fixture, peer, grant, id)
}
async fn read(
    state: &Arc<AppState>,
    peer: &Peer,
    grant: &str,
    id: &str,
) -> axum::response::Response {
    origin(
        peer.origin.clone(),
        call(
            state,
            PATH,
            json!({"providerId":"dim","credentialId":id,
        "expectedRevision":state.route_config.snapshot().revision().id()}),
            Some(grant),
        ),
    )
    .await
}
async fn wait_reads(peer: &Peer, count: usize) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while peer.catalog_reads.load(Ordering::SeqCst) < count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn dim_catalog_read_is_not_a_write_and_normal_cas_keeps_secrets_and_exact_wire() {
    let (fixture, peer, grant, id) = saved().await;
    let state = &fixture.state;
    let before = state.route_config.snapshot();
    *peer.catalog.lock().unwrap() =
        json!({"data":[{"id":"opaque-new","dim":{"endpoint_type":"openai-response"}}]});
    let response = read(state, &peer, &grant, &id).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let result = support::parse_json(response).await;
    assert_redacted(&result);
    assert_eq!(result["revision"], before.revision().id());
    assert_eq!(result["providerId"], "dim");
    assert_eq!(result["credentialId"], id);
    assert_eq!(result["catalog"]["models"][0]["protocol"], "responses");
    assert_eq!(
        state.route_config.snapshot().revision().id(),
        before.revision().id()
    );
    assert_eq!(
        state.route_config.snapshot().document().providers[0].credentials[0]
            .dim_catalog
            .as_ref()
            .unwrap()
            .models()
            .len(),
        4
    );
    let redacted = crate::console::secrets::redact_route_document(before.document()).unwrap();
    let mut draft = serde_json::to_value(redacted.document).unwrap();
    draft["providers"][0]["credentials"][0]["dim_catalog"] = result["catalog"].clone();
    draft["providers"][0]["credentials"][0]["supported_models"] =
        json!(["operator-model", "opaque-new"]);
    draft["providers"][0]["credentials"][0]["model_selection"] =
        json!({"manual":["operator-model"],"fetched":["opaque-new"]});
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| json!({"path":s.path,"operation":"keep"}))
        .collect();
    let body =
        json!({"expectedRevision":before.revision().id(),"document":draft,"secretPatches":patches});
    let response = support::send_console_json(
        state,
        Method::PUT,
        "/v1/internal/gateway/console/route-config",
        body.clone(),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    let after = state.route_config.snapshot();
    let credential = &after.document().providers[0].credentials[0];
    assert_eq!(credential.api_key.as_deref(), Some("synthetic-dim-access"));
    assert_eq!(
        credential.refresh_token.as_deref(),
        Some("synthetic-dim-refresh")
    );
    assert_eq!(
        after.document().providers[1].api_key,
        "synthetic-unrelated-secret"
    );
    assert_eq!(
        credential.model_selection.as_ref().unwrap().manual,
        ["operator-model"]
    );
    assert_eq!(
        after.resolve_candidates(Some("opaque-new"))[0].protocol_family,
        "openai_responses"
    );
    assert!(after
        .resolve_candidates(Some("opaque-chat"))
        .iter()
        .all(|candidate| candidate.provider_account_id != "dim"));
    let response = support::send_console_json(
        state,
        Method::PUT,
        "/v1/internal/gateway/console/route-config",
        body,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(peer.count(), 1);
    assert_eq!(peer.catalog_reads.load(Ordering::SeqCst), 2);
    assert!(peer.inference.lock().unwrap().is_empty());
    state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}
#[tokio::test]
async fn dim_catalog_failed_or_stale_read_cannot_publish_or_return_a_foreign_snapshot() {
    let (fixture, peer, grant, id) = saved().await;
    let state = &fixture.state;
    let before = state.route_config.snapshot();
    peer.catalog_status.store(503, Ordering::SeqCst);
    let response = read(state, &peer, &grant, &id).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        state.route_config.snapshot().revision().id(),
        before.revision().id()
    );
    peer.catalog_status.store(200, Ordering::SeqCst);
    peer.catalog_blocked.store(true, Ordering::SeqCst);
    let (response, ()) = tokio::join!(read(state, &peer, &grant, &id), async {
        wait_reads(&peer, 3).await;
        let mut draft = before.document().clone();
        draft.providers[0].credentials[0].account_name = Some("operator-edit".into());
        state
            .route_config_runtime
            .as_ref()
            .unwrap()
            .commit_document(before.revision().id(), draft, None)
            .await
            .unwrap();
        peer.catalog_release.notify_one();
    });
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let result = support::parse_json(response).await;
    assert_redacted(&result);
    assert!(result.get("catalog").is_none());
    assert_eq!(peer.count(), 1);
    assert!(peer.inference.lock().unwrap().is_empty());
    state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}
#[tokio::test]
async fn dim_catalog_rechecks_sensitive_consent_after_the_network_read() {
    let (fixture, peer, grant, id) = saved().await;
    let state = &fixture.state;
    let before = state.route_config.snapshot().revision().id().to_owned();
    peer.catalog_blocked.store(true, Ordering::SeqCst);
    let (response, ()) = tokio::join!(read(state, &peer, &grant, &id), async {
        wait_reads(&peer, 2).await;
        let request = crate::http::routes::internal_console::console_request_context(
            &axum::http::HeaderMap::new(),
            None,
        );
        state
            .console_auth
            .logout(&request, support::MANAGEMENT_TOKEN)
            .unwrap();
        peer.catalog_release.notify_one();
    });
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(state.route_config.snapshot().revision().id(), before);
    assert_eq!(peer.count(), 1);
    assert!(peer.inference.lock().unwrap().is_empty());
    state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}
