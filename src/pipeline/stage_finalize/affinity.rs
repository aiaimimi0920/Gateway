//! Success-only affinity shared by buffered completion and terminal stream completion.
use super::*;

pub(super) struct SuccessAffinity<'a> {
    pub provider: Option<&'a str>,
    pub model: Option<&'a str>,
    pub session: Option<&'a str>,
    pub user: Option<&'a str>,
    pub access_key: Option<&'a str>,
    pub source_key: Option<&'a str>,
    pub platform: Option<&'a str>,
    pub credential: Option<&'a str>,
}

impl SuccessAffinity<'_> {
    pub async fn record(self, state: &Arc<AppState>) {
        let (Some(provider), Some(model)) = (self.provider, self.model) else {
            return;
        };
        if let (Some(access), Some(source), Some(platform)) =
            (self.access_key, self.source_key, self.platform)
        {
            let _ = db::record_access_sticky_affinity(
                &state.redis_pool,
                access,
                source,
                platform,
                provider,
                self.credential,
                model,
                self.session,
            )
            .await;
        } else {
            // These are the same scoped keys read by stage_route, not global preference.
            if let Some(session) = self.session {
                set_affinity(state, &format!("session:{session}"), model, provider).await;
            }
            if let Some(user) = self.user {
                set_affinity(state, &format!("user:{user}"), model, provider).await;
            }
        }
    }
}

pub(super) async fn record_stream(snapshot: &RequestAuditFinalizeSnapshot, state: &Arc<AppState>) {
    SuccessAffinity {
        provider: snapshot.provider_account_id.as_deref(),
        model: snapshot
            .requested_model
            .as_deref()
            .or(snapshot.resolved_model.as_deref()),
        session: snapshot.session_id.as_deref(),
        user: snapshot.user_id.as_deref(),
        access_key: snapshot.access_key_id.as_deref(),
        source_key: snapshot.source_access_key_id.as_deref(),
        platform: snapshot.platform_access_id.as_deref(),
        credential: snapshot.real_credential_ref.as_deref(),
    }
    .record(state)
    .await;
}
