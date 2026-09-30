//! Storage-aware readiness data and policy, shared by public management projections.
use crate::db::GatewayReadinessProviderStatsView;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DependencyReadiness {
    pub configured: bool,
    pub required: bool,
    pub ready: bool,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessProbeOutcome {
    pub ready: bool,
    pub timed_out: bool,
}

#[derive(Debug, Clone)]
pub(super) struct GatewayRuntimeReadiness {
    pub(super) redis: DependencyReadiness,
    pub(super) postgresql: DependencyReadiness,
    pub(super) sqlite: Option<DependencyReadiness>,
    pub(super) object_storage: DependencyReadiness,
    pub(super) object_storage_driver: String,
    pub(super) api_key_secret: bool,
    pub(super) public_base_url: bool,
    pub(super) provider_stats: GatewayReadinessProviderStatsView,
}

impl GatewayRuntimeReadiness {
    pub(super) fn overall_ready(&self, draining: bool) -> bool {
        if let Some(sqlite) = &self.sqlite {
            return sqlite.ready && self.object_storage.ready && !draining;
        }
        // Server management retains its enterprise configuration gates.
        self.redis.ready
            && self.postgresql.ready
            && self.object_storage.ready
            && self.api_key_secret
            && self.public_base_url
            && !draining
    }
}

pub fn optional_postgresql_readiness(configured: bool, probe_ready: bool) -> DependencyReadiness {
    DependencyReadiness {
        configured,
        required: configured,
        ready: !configured || probe_ready,
        timed_out: false,
    }
}
