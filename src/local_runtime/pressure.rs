//! Local pressure joins durable running requests with live admission counters.
use super::LocalRuntime;
use crate::{concurrency::aimd::ConcurrencySnapshot, db::*, error::GatewayError};
use std::collections::HashMap;

impl LocalRuntime {
    pub async fn pressure(
        &self,
        snapshots: &HashMap<String, ConcurrencySnapshot>,
        identities: &[GatewayRuntimeProviderIdentity],
        filters: &GatewayRuntimePressureFilters,
    ) -> Result<GatewayRuntimePressureView, GatewayError> {
        let rows: Vec<(String, Option<String>, i64)> = sqlx::query_as(
            "SELECT json_extract(payload, '$.projectId'), json_extract(payload, '$.providerAccountId'), count(*)
             FROM request_audits WHERE status = 'running'
             AND (? IS NULL OR json_extract(payload, '$.projectId') = ?)
             AND (? IS NULL OR json_extract(payload, '$.providerAccountId') = ?)
             GROUP BY json_extract(payload, '$.projectId'), json_extract(payload, '$.providerAccountId')")
            .bind(&filters.project_id).bind(&filters.project_id)
            .bind(&filters.provider_account_id).bind(&filters.provider_account_id)
            .fetch_all(&self.pool).await.map_err(super::storage_error)?;
        let mut project_counts = HashMap::<String, usize>::new();
        let mut provider_counts = HashMap::<String, usize>::new();
        for (project, provider, count) in &rows {
            *project_counts.entry(project.clone()).or_default() += *count as usize;
            if let Some(id) = provider {
                *provider_counts.entry(id.clone()).or_default() += *count as usize;
            }
        }
        let mut projects = project_counts
            .into_iter()
            .map(|(id, count)| GatewayProjectPressureView {
                display_name: id.clone(),
                project_id: id,
                active_concurrency: count,
                running_request_count: count,
            })
            .collect::<Vec<_>>();
        projects.sort_by(|a, b| {
            b.active_concurrency
                .cmp(&a.active_concurrency)
                .then_with(|| a.project_id.cmp(&b.project_id))
        });
        let mut identities = identities.to_vec();
        for id in provider_counts.keys() {
            if !identities.iter().any(|provider| &provider.id == id) {
                identities.push(GatewayRuntimeProviderIdentity {
                    id: id.clone(),
                    label: id.clone(),
                    status: "unknown".into(),
                    adapter: String::new(),
                    protocol_family: String::new(),
                    supported_models: Vec::new(),
                });
            }
        }
        let mut providers = identities
            .iter()
            .filter(|provider| {
                filters
                    .provider_account_id
                    .as_ref()
                    .is_none_or(|id| id == &provider.id)
            })
            .map(|provider| {
                let snapshot = snapshots.get(&provider.id);
                let running = provider_counts.get(&provider.id).copied().unwrap_or(0);
                GatewayProviderPressureView {
                    provider_account_id: provider.id.clone(),
                    label: provider.label.clone(),
                    status: provider.status.clone(),
                    protocol_family: provider.protocol_family.clone(),
                    active_concurrency: if filters.project_id.is_some() {
                        running
                    } else {
                        snapshot.map_or(running, |s| s.active_count)
                    },
                    concurrency_limit: snapshot.map(|s| s.current_limit),
                    concurrency_available: snapshot.map(|s| s.available),
                    running_request_count: running,
                    breaker_open: false,
                }
            })
            .collect::<Vec<_>>();
        providers.sort_by(|a, b| {
            b.active_concurrency
                .cmp(&a.active_concurrency)
                .then_with(|| a.label.cmp(&b.label))
        });
        Ok(GatewayRuntimePressureView {
            total_running_requests: rows.iter().map(|(_, _, count)| *count as usize).sum(),
            total_project_concurrency: projects.iter().map(|p| p.active_concurrency).sum(),
            total_provider_concurrency: providers.iter().map(|p| p.active_concurrency).sum(),
            projects,
            providers,
        })
    }
}
