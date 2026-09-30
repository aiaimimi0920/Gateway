use super::*;

#[cfg(test)]
#[path = "summary_tests.rs"]
mod tests;

#[derive(Default)]
struct ProviderWindowAccumulator {
    total_requests: usize,
    success_count: usize,
    failure_count: usize,
}

#[derive(Default)]
struct ProviderStatsAccumulator {
    total_requests: usize,
    completed_count: usize,
    failed_count: usize,
    cancelled_count: usize,
    running_count: usize,
    last_request_at: Option<String>,
    /// Keyed by `YYYY-MM-DDTHH` so the BTreeMap keeps chronological order.
    windows: BTreeMap<String, ProviderWindowAccumulator>,
    /// Keyed by upstream model name; empty for the per-model accumulators
    /// themselves so the nesting stays one level deep.
    models: BTreeMap<String, ProviderStatsAccumulator>,
}

impl ProviderStatsAccumulator {
    fn accumulate(&mut self, row: &GatewayRequestAuditView) {
        self.accumulate_totals(row);
        if let Some(model) = audit_row_model(row) {
            self.models
                .entry(model.to_string())
                .or_default()
                .accumulate_totals(row);
        }
    }

    fn accumulate_totals(&mut self, row: &GatewayRequestAuditView) {
        self.total_requests += 1;
        match row.status.as_str() {
            "completed" => self.completed_count += 1,
            "failed" => self.failed_count += 1,
            "cancelled" => self.cancelled_count += 1,
            "running" => self.running_count += 1,
            _ => {}
        }

        if self
            .last_request_at
            .as_deref()
            .is_none_or(|current| row.created_at.as_str() > current)
        {
            self.last_request_at = Some(row.created_at.clone());
        }

        if let Some(hour_key) = hour_bucket_key(&row.created_at) {
            let window = self.windows.entry(hour_key).or_default();
            window.total_requests += 1;
            match row.status.as_str() {
                "completed" => window.success_count += 1,
                "failed" => window.failure_count += 1,
                _ => {}
            }
        }
    }

    fn into_view(self, provider_account_id: String) -> GatewayRequestAuditProviderStatsView {
        GatewayRequestAuditProviderStatsView {
            provider_account_id,
            total_requests: self.total_requests,
            completed_count: self.completed_count,
            failed_count: self.failed_count,
            cancelled_count: self.cancelled_count,
            running_count: self.running_count,
            last_request_at: self.last_request_at,
            windows: window_views(&self.windows),
            models: self
                .models
                .into_iter()
                .map(|(model, accumulator)| accumulator.into_model_view(model))
                .collect(),
        }
    }

    fn into_model_view(self, model: String) -> GatewayRequestAuditProviderModelStatsView {
        GatewayRequestAuditProviderModelStatsView {
            model,
            total_requests: self.total_requests,
            completed_count: self.completed_count,
            failed_count: self.failed_count,
            cancelled_count: self.cancelled_count,
            running_count: self.running_count,
            last_request_at: self.last_request_at,
            windows: window_views(&self.windows),
        }
    }
}

pub async fn summarize_request_audits(
    pool: &PgPool,
    filters: &RequestAuditFilters,
) -> Result<GatewayRequestAuditSummaryView, GatewayError> {
    let rows = list_request_audits(pool, filters).await?;
    Ok(summarize_request_audit_rows(&rows))
}

pub fn summarize_request_audit_rows(
    rows: &[GatewayRequestAuditView],
) -> GatewayRequestAuditSummaryView {
    let mut by_status = BTreeMap::new();
    let mut by_provider_account = BTreeMap::new();
    let mut by_endpoint_kind = BTreeMap::new();
    let mut by_error_code = BTreeMap::new();

    let mut completed_count = 0;
    let mut failed_count = 0;
    let mut cancelled_count = 0;
    let mut running_count = 0;
    let mut fallback_eligible_failures = 0;
    let mut fallback_exhausted_failures = 0;
    let mut provider_stats: BTreeMap<String, ProviderStatsAccumulator> = BTreeMap::new();
    let mut credential_stats: BTreeMap<(String, String), ProviderStatsAccumulator> =
        BTreeMap::new();

    for row in rows {
        accumulate_bucket(&mut by_status, Some(row.status.as_str()));
        accumulate_bucket(&mut by_provider_account, row.provider_account_id.as_deref());
        accumulate_bucket(&mut by_endpoint_kind, Some(row.endpoint_kind.as_str()));
        accumulate_bucket(
            &mut by_error_code,
            route_trace_error_code(row.route_trace.as_ref()),
        );

        match row.status.as_str() {
            "completed" => completed_count += 1,
            "failed" => {
                failed_count += 1;
                if route_trace_fallback_eligible(row.route_trace.as_ref()) {
                    fallback_eligible_failures += 1;
                } else {
                    fallback_exhausted_failures += 1;
                }
            }
            "cancelled" => cancelled_count += 1,
            "running" => running_count += 1,
            _ => {}
        }

        if let Some(provider_account_id) = row
            .provider_account_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            provider_stats
                .entry(provider_account_id.to_string())
                .or_default()
                .accumulate(row);
            // Missing attribution in older audits must never be guessed from pool size.
            if let Some(credential_ref) = row
                .route_trace
                .as_ref()
                .and_then(|trace| trace.get("realCredentialRef"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                credential_stats
                    .entry((provider_account_id.to_string(), credential_ref.to_string()))
                    .or_default()
                    .accumulate(row);
            }
        }
    }

    let provider_accounts = provider_stats
        .into_iter()
        .map(|(provider_account_id, stats)| stats.into_view(provider_account_id))
        .collect();

    GatewayRequestAuditSummaryView {
        total_requests: rows.len(),
        completed_count,
        failed_count,
        cancelled_count,
        running_count,
        fallback_eligible_failures,
        fallback_exhausted_failures,
        by_status: into_summary_buckets(by_status),
        by_provider_account: into_summary_buckets(by_provider_account),
        by_endpoint_kind: into_summary_buckets(by_endpoint_kind),
        by_error_code: into_summary_buckets(by_error_code),
        provider_accounts,
        credentials: credential_stats
            .into_iter()
            .map(|((provider_account_id, credential_ref), stats)| {
                GatewayRequestAuditCredentialStatsView {
                    credential_ref,
                    stats: stats.into_view(provider_account_id),
                }
            })
            .collect(),
    }
}

fn route_trace_error_code(route_trace: Option<&Value>) -> Option<&str> {
    route_trace
        .and_then(|value| value.get("errorCode"))
        .and_then(Value::as_str)
}

fn route_trace_fallback_eligible(route_trace: Option<&Value>) -> bool {
    route_trace
        .and_then(|value| value.get("fallbackEligible"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// The name the cost overview aggregates by, so audit stats and cost rows share
/// one key: the model the upstream was actually called with, falling back to
/// what the client asked for.
fn audit_row_model(row: &GatewayRequestAuditView) -> Option<&str> {
    [
        row.resolved_model.as_deref(),
        row.requested_model.as_deref(),
        row.model_alias.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .find(|value| !value.is_empty())
}

fn window_views(
    windows: &BTreeMap<String, ProviderWindowAccumulator>,
) -> Vec<GatewayRequestAuditProviderWindowView> {
    windows
        .iter()
        .map(|(hour_key, window)| GatewayRequestAuditProviderWindowView {
            label: hour_bucket_label(hour_key),
            bucket_start: format!("{hour_key}:00:00Z"),
            total_requests: window.total_requests,
            success_count: window.success_count,
            failure_count: window.failure_count,
        })
        .collect()
}

/// `2026-08-21T13:45:12.123Z` -> `2026-08-21T13`. Returns `None` for anything
/// that is not an ISO-8601 timestamp with an hour component.
fn hour_bucket_key(created_at: &str) -> Option<String> {
    let trimmed = created_at.trim();
    if trimmed.len() < 13 || !trimmed.is_char_boundary(13) {
        return None;
    }
    let candidate = &trimmed[..13];
    let bytes = candidate.as_bytes();
    if bytes[10] != b'T' && bytes[10] != b' ' {
        return None;
    }
    if !bytes[11].is_ascii_digit() || !bytes[12].is_ascii_digit() {
        return None;
    }
    Some(format!("{}T{}", &candidate[..10], &candidate[11..13]))
}

fn hour_bucket_label(hour_key: &str) -> String {
    match hour_key.split_once('T') {
        Some((_, hour)) => format!("{hour}:00"),
        None => hour_key.to_string(),
    }
}
