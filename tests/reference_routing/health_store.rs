//! The same health contract runs against SQLite and a guarded disposable PG.
use neuro_gateway::routing::candidate::RouteCandidate;
use neuro_gateway::{db, local_runtime::LocalRuntime};
use sqlx::PgPool;

pub enum HealthStore<'a> {
    Local(&'a LocalRuntime),
    Pg(&'a PgPool),
}

impl HealthStore<'_> {
    pub async fn failure(
        &self,
        input: db::RecordCredentialModelFailureInput,
    ) -> db::GatewayProviderCredentialModelStateView {
        match self {
            Self::Local(local) => local.model_failure(input).await,
            Self::Pg(pool) => db::record_provider_credential_model_failure(pool, input).await,
        }
        .unwrap()
        .unwrap()
    }

    pub async fn success(
        &self,
        input: db::RecordCredentialModelSuccessInput,
    ) -> db::GatewayProviderCredentialModelStateView {
        match self {
            Self::Local(local) => local.model_success(input).await,
            Self::Pg(pool) => db::record_provider_credential_model_success(pool, input).await,
        }
        .unwrap()
    }

    pub async fn apply(&self, candidates: &mut [RouteCandidate]) {
        match self {
            Self::Local(local) => local.apply_model_states(candidates).await,
            Self::Pg(pool) => {
                db::apply_provider_credential_model_states_to_candidates(pool, candidates).await
            }
        }
        .unwrap()
    }

    pub async fn list(
        &self,
        filters: db::CredentialModelStateFilters,
    ) -> Vec<db::GatewayProviderCredentialModelStateView> {
        match self {
            Self::Local(local) => local.list_model_states(filters).await,
            Self::Pg(pool) => db::list_provider_credential_model_states(pool, filters).await,
        }
        .unwrap()
    }
}

pub async fn contract(store: HealthStore<'_>, candidate: RouteCandidate) {
    for ref_only in [false, true] {
        let mut candidate = candidate.clone();
        candidate.provider_credential_id = if ref_only {
            None
        } else {
            Some("credential-one".into())
        };
        candidate.payload.credential_id = Some("real-ref".into());
        identity_contract(&store, candidate).await;
    }
}

async fn identity_contract(store: &HealthStore<'_>, mut candidate: RouteCandidate) {
    use time::{format_description::well_known::Rfc3339, OffsetDateTime};
    for (index, hint, expected) in [
        (0, None, Some(300)),
        (1, Some(0), Some(0)),
        (2, Some(1), Some(1)),
        (3, Some(1_001), Some(2)),
        (4, Some(172_800_000), Some(172_800)),
        (5, Some(u64::MAX), None),
    ] {
        let model = format!(
            "upstream-model-{}-{index}",
            candidate.provider_credential_id.is_some()
        );
        candidate.upstream_model = Some(model.clone());
        let input = db::RecordCredentialModelFailureInput {
            provider_account_id: candidate.provider_account_id.clone(),
            provider_credential_id: candidate.provider_credential_id.clone(),
            provider_credential_ref: candidate.payload.credential_id.clone(),
            protocol_profile: Some(candidate.protocol_profile.clone()),
            model: model.clone(),
            upstream_status: Some(429),
            upstream_retry_after_ms: hint,
            error_message: Some("rate limit".into()),
            classification: neuro_gateway::provider_failure::classify_provider_failure(
                Some(429),
                None,
                Some("rate limit"),
            ),
        };
        let first = store.failure(input.clone()).await;
        let row = store.failure(input).await;
        assert_eq!(row.id, first.id);
        assert_eq!(row.failure_count, 2);
        let until =
            OffsetDateTime::parse(row.cooldown_until.as_deref().unwrap(), &Rfc3339).unwrap();
        if let Some(seconds) = expected {
            let actual = (until - OffsetDateTime::now_utc()).whole_seconds();
            assert!(
                (seconds - 2..=seconds + 1).contains(&actual),
                "hint={hint:?}, actual={actual}"
            );
        } else {
            assert_eq!(until.year(), 9999);
        }
        let rows = store
            .list(db::CredentialModelStateFilters {
                model: Some(model.clone()),
                ..Default::default()
            })
            .await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cooldown_until, row.cooldown_until);
        let mut other_model = candidate.clone();
        other_model.upstream_model = Some("unaffected".into());
        let mut other_credential = candidate.clone();
        if other_credential.provider_credential_id.is_some() {
            other_credential.provider_credential_id = Some("credential-two".into());
        } else {
            other_credential.payload.credential_id = Some("other-ref".into());
        }
        let mut candidates = [candidate.clone(), other_model, other_credential];
        store.apply(&mut candidates).await;
        assert_eq!(candidates[0].cooldown_until, row.cooldown_until);
        assert_eq!(candidates[0].failure_count, 2);
        assert!(candidates[1].cooldown_until.is_none());
        assert!(candidates[2].cooldown_until.is_none());
        let cleared = store
            .success(db::RecordCredentialModelSuccessInput {
                provider_account_id: candidate.provider_account_id.clone(),
                provider_credential_id: candidate.provider_credential_id.clone(),
                provider_credential_ref: candidate.payload.credential_id.clone(),
                protocol_profile: Some(candidate.protocol_profile.clone()),
                model,
            })
            .await;
        assert_eq!(cleared.id, row.id);
        assert_eq!(cleared.status, "active");
        assert_eq!(cleared.failure_count, 0);
        assert!(cleared.cooldown_until.is_none());
        let mut active = [candidate.clone()];
        store.apply(&mut active).await;
        assert!(active[0].cooldown_until.is_none());
    }
}
