//! Reuse the joined maintenance loop; checks never POST creative actions or repeat uncertain work.
use crate::state::AppState;

pub(crate) async fn check(state: &AppState) {
    if let Some(store) = state.upstream_client.luma_web_sessions.get() {
        store.flush_pending().await;
    }
    let snapshot = state.route_config.snapshot();
    for provider in snapshot.get_providers() {
        let mut targets = Vec::new();
        if provider.uses_provider_default_account() {
            targets.push(provider.payload);
        } else {
            targets.extend(
                provider
                    .credential_pool
                    .into_iter()
                    .filter(|c| c.enabled)
                    .map(|c| c.payload),
            );
        }
        for payload in targets {
            if state.shutdown.is_requested() || state.lifecycle.is_draining() {
                return;
            }
            if payload.adapter != "lumalabs_compatible" || payload.api_key.trim().is_empty() {
                continue;
            }
            if let Err(error) = state
                .upstream_client
                .maintain_luma_web_session(&provider.id, &payload)
                .await
            {
                tracing::debug!(provider = %provider.id, code = ?error.code, "Luma Web session maintenance deferred");
            }
        }
    }
}
