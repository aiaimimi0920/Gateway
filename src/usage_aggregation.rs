use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::redis::usage_tracking::UsageReport;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageAggregateBucket {
    pub bucket_start: String,
    pub bucket_granularity: String,
    pub project_id: String,
    pub user_id: String,
    pub provider: String,
    pub provider_credential_ref: String,
    pub model: String,
    pub request_count: u64,
    pub failure_count: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub latency_ms_sum: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct UsageAggregateKey {
    bucket_start: String,
    project_id: String,
    user_id: String,
    provider: String,
    provider_credential_ref: String,
    model: String,
}

pub fn aggregate_usage_reports_by_user_credential_model(
    reports: &[UsageReport],
    bucket_seconds: i64,
) -> Vec<UsageAggregateBucket> {
    let bucket_seconds = bucket_seconds.max(60);
    let mut buckets = BTreeMap::<UsageAggregateKey, UsageAggregateBucket>::new();

    for report in reports {
        let bucket_start = bucket_start_for_report(report, bucket_seconds);
        let key = UsageAggregateKey {
            bucket_start: bucket_start.clone(),
            project_id: report.project_id.clone(),
            user_id: report.user_id.clone(),
            provider: report.provider.clone(),
            provider_credential_ref: report.credential_id.clone(),
            model: report.model.clone(),
        };
        let bucket = buckets.entry(key).or_insert_with(|| UsageAggregateBucket {
            bucket_start,
            bucket_granularity: format!("{bucket_seconds}s"),
            project_id: report.project_id.clone(),
            user_id: report.user_id.clone(),
            provider: report.provider.clone(),
            provider_credential_ref: report.credential_id.clone(),
            model: report.model.clone(),
            request_count: 0,
            failure_count: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            latency_ms_sum: 0,
        });

        bucket.request_count = bucket.request_count.saturating_add(1);
        if !report.success {
            bucket.failure_count = bucket.failure_count.saturating_add(1);
        }
        bucket.prompt_tokens = bucket.prompt_tokens.saturating_add(report.prompt_tokens);
        bucket.completion_tokens = bucket
            .completion_tokens
            .saturating_add(report.completion_tokens);
        bucket.total_tokens = bucket.total_tokens.saturating_add(report.total_tokens);
        bucket.cache_creation_input_tokens = bucket
            .cache_creation_input_tokens
            .saturating_add(report.cache_creation_input_tokens.unwrap_or(0));
        bucket.cache_read_input_tokens = bucket
            .cache_read_input_tokens
            .saturating_add(report.cache_read_input_tokens.unwrap_or(0));
        bucket.latency_ms_sum = bucket.latency_ms_sum.saturating_add(report.latency_ms);
    }

    buckets.into_values().collect()
}

fn bucket_start_for_report(report: &UsageReport, bucket_seconds: i64) -> String {
    let timestamp = OffsetDateTime::parse(&report.request_completed_at, &Rfc3339)
        .or_else(|_| OffsetDateTime::parse(&report.request_started_at, &Rfc3339))
        .unwrap_or_else(|_| OffsetDateTime::UNIX_EPOCH);
    let unix = timestamp.unix_timestamp();
    let bucket_unix = unix - unix.rem_euclid(bucket_seconds);
    OffsetDateTime::from_unix_timestamp(bucket_unix)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .format(&Rfc3339)
        .unwrap_or_else(|_| bucket_unix.to_string())
}
