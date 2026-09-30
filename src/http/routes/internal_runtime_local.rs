//! Local readiness probes SQLite and object storage, without server configuration gates.
use super::*;

pub(super) async fn probe(
    state: &AppState,
    runtime: &crate::local_runtime::LocalRuntime,
    deadline: TokioInstant,
) -> GatewayRuntimeReadiness {
    let (sqlite, objects) = tokio::join!(
        bounded_readiness_outcome_at(deadline, async {
            ReadinessProbeOutcome {
                ready: sqlx::query("SELECT 1").execute(&runtime.pool).await.is_ok(),
                timed_out: false,
            }
        }),
        bounded_readiness_outcome_at(deadline, async {
            match crate::object_storage::GatewayObjectStorage::from_env() {
                Ok(storage) => {
                    let outcome = storage
                        .probe_readiness(deadline.saturating_duration_since(TokioInstant::now()))
                        .await;
                    ReadinessProbeOutcome {
                        ready: outcome.ready,
                        timed_out: outcome.timed_out,
                    }
                }
                Err(_) => ReadinessProbeOutcome {
                    ready: false,
                    timed_out: false,
                },
            }
        })
    );
    let dependency = |outcome: ReadinessProbeOutcome| DependencyReadiness {
        configured: true,
        required: true,
        ready: outcome.ready,
        timed_out: outcome.timed_out,
    };
    GatewayRuntimeReadiness {
        redis: DependencyReadiness {
            configured: false,
            required: false,
            ready: true,
            timed_out: false,
        },
        postgresql: optional_postgresql_readiness(false, false),
        sqlite: Some(dependency(sqlite)),
        object_storage: dependency(objects),
        object_storage_driver: object_storage_driver(),
        api_key_secret: state
            .config
            .gateway_api_key_secret
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        public_base_url: public_base_url_configured(),
        provider_stats: db::GatewayReadinessProviderStatsView::default(),
    }
}
