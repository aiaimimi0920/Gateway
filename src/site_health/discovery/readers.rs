//! Internal health capabilities, not site-specific adapters or proof of gateway identity.
use super::HealthAdapter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reader {
    PerfSummary,
    ModelStatus,
    ChannelHealth,
    Embed,
    Slot,
    Matrix,
}

impl Reader {
    pub(super) const ALL: [Self; 6] = [
        Self::PerfSummary,
        Self::ModelStatus,
        Self::ChannelHealth,
        Self::Embed,
        Self::Slot,
        Self::Matrix,
    ];

    pub(super) fn recipe(self) -> (&'static str, &'static str) {
        match self {
            Self::PerfSummary => ("new-api-perf-summary", "api/perf-metrics/summary?hours=24"),
            Self::ModelStatus => (
                "model-status-all",
                "api/enhancements/model-status/embed/status/all",
            ),
            Self::ChannelHealth => ("channel-health", "api/channel/health/models?days=1"),
            Self::Embed => (
                "model-status-embed",
                "api/model-status/embed/config/selected",
            ),
            Self::Slot => ("model-health-slot", "api/model_health/slot_status"),
            Self::Matrix => (
                "sub2api-v2",
                "api/v1/channel-monitor-v2/matrix?range=24h&group_by=platform_group_model",
            ),
        }
    }

    pub(super) fn legacy(adapter: HealthAdapter) -> Option<Self> {
        Some(match adapter {
            HealthAdapter::NewApiPerfSummary => Self::PerfSummary,
            HealthAdapter::ModelStatusAll => Self::ModelStatus,
            HealthAdapter::ChannelHealth => Self::ChannelHealth,
            HealthAdapter::ModelStatusEmbed => Self::Embed,
            HealthAdapter::ModelHealthSlot => Self::Slot,
            HealthAdapter::Sub2apiV2 => Self::Matrix,
            _ => return None,
        })
    }

    pub(super) fn candidates(adapter: HealthAdapter) -> Vec<Self> {
        if let Some(reader) = Self::legacy(adapter) {
            return vec![reader];
        }
        match adapter {
            // One API has no confirmed core model-health API. These are optional
            // compatible capabilities, never a promise made by the upstream project.
            HealthAdapter::NewApi | HealthAdapter::OneApi => Self::ALL[..5].to_vec(),
            HealthAdapter::Sub2api => vec![Self::Matrix],
            HealthAdapter::Auto | HealthAdapter::CompatibleGateway => Self::ALL.to_vec(),
            _ => Vec::new(),
        }
    }

    pub(super) fn authenticated_default(adapter: HealthAdapter) -> Option<Self> {
        match adapter {
            HealthAdapter::NewApi => Some(Self::PerfSummary),
            HealthAdapter::Sub2api => Some(Self::Matrix),
            _ => None,
        }
    }
}
