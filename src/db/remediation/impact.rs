use super::execution::append_incident_history;
use super::runs::{
    get_remediation_run_row_by_id, get_remediation_run_view_by_id, to_remediation_run_view,
};
use super::*;

pub async fn get_anomaly_incident_remediation_run_impact(
    pool: &PgPool,
    run_id: &str,
    window_minutes: Option<i32>,
) -> Result<GatewayAnalysisAnomalyRemediationRunImpactView, GatewayError> {
    let run = get_remediation_run_view_by_id(pool, run_id).await?;
    let incident = run.after_incident.clone().or(run.before_incident.clone());
    let project_id = incident
        .as_ref()
        .and_then(|value| value.project_id.clone())
        .ok_or_else(|| {
            GatewayError::conflict("当前 remediation run 缺少 project 作用域，无法计算影响面。")
        })?;
    let route_policy_id = run
        .after_route_policy
        .as_ref()
        .map(|value| value.id.clone())
        .or(run
            .before_route_policy
            .as_ref()
            .map(|value| value.id.clone()))
        .or(run.route_policy_id.clone());
    let anchor_at = parse_filter_timestamp(
        run.completed_at
            .as_deref()
            .or(Some(run.created_at.as_str())),
        "anchorAt",
    )?
    .ok_or_else(|| GatewayError::conflict("当前 remediation run 缺少合法的时间锚点。"))?;
    let window_minutes = normalize_window_minutes(window_minutes);

    let before_started_at = anchor_at - time::Duration::minutes(i64::from(window_minutes));
    let before_ended_at = anchor_at;
    let after_started_at = anchor_at;
    let after_ended_at = anchor_at + time::Duration::minutes(i64::from(window_minutes));

    let before_summary = summarize_analysis(
        pool,
        &RequestAuditFilters {
            project_id: Some(project_id.clone()),
            route_policy_id: route_policy_id.clone(),
            created_from: Some(format_timestamp(before_started_at)),
            created_to: Some(format_timestamp(before_ended_at)),
            limit: Some(1000),
            ..RequestAuditFilters::default()
        },
    )
    .await?;
    let after_summary = summarize_analysis(
        pool,
        &RequestAuditFilters {
            project_id: Some(project_id.clone()),
            route_policy_id: route_policy_id.clone(),
            created_from: Some(format_timestamp(after_started_at)),
            created_to: Some(format_timestamp(after_ended_at)),
            limit: Some(1000),
            ..RequestAuditFilters::default()
        },
    )
    .await?;

    Ok(build_run_impact(
        OffsetDateTime::now_utc(),
        run,
        incident,
        Some(project_id),
        route_policy_id,
        anchor_at,
        window_minutes,
        before_started_at,
        before_ended_at,
        before_summary,
        after_started_at,
        after_ended_at,
        after_summary,
    ))
}

pub async fn capture_anomaly_incident_remediation_run_impact(
    pool: &PgPool,
    actor_user_id: &str,
    run_id: &str,
    window_minutes: Option<i32>,
) -> Result<GatewayAnalysisAnomalyRemediationImpactCaptureView, GatewayError> {
    let actor_user_id = trimmed_owned_ref(actor_user_id).unwrap_or("management");
    let impact = get_anomaly_incident_remediation_run_impact(pool, run_id, window_minutes).await?;
    let row = get_remediation_run_row_by_id(pool, &impact.run.id).await?;

    let mut result_payload = row.result.map(|value| value.0).unwrap_or_else(|| json!({}));
    if !result_payload.is_object() {
        result_payload = json!({});
    }
    if let Some(object) = result_payload.as_object_mut() {
        object.insert(
            "impactCapture".to_string(),
            json!({
                "capturedAt": impact.generated_at,
                "windowMinutes": impact.window_minutes,
                "impact": impact
            }),
        );
    }

    let updated_row = sqlx::query_as::<_, GatewayAnalysisAnomalyRemediationRunRow>(
        r#"
        update gateway_analysis_anomaly_remediation_runs
        set result = $2
        where id = $1
        returning
          id, incident_id, policy_id, route_policy_id, action_key, title, execution_mode, status, dry_run,
          actor_user_id, note, input, result, before_incident, after_incident, before_route_policy, after_route_policy,
          error_summary, created_at, completed_at
        "#,
    )
    .bind(&impact.run.id)
    .bind(sqlx::types::Json(result_payload))
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    if let Some(incident) = impact.incident.as_ref() {
        append_incident_history(
            pool,
            &impact.run.incident_id,
            "remediation_impact_captured",
            Some(actor_user_id),
            Some(&format!(
                "Captured remediation impact over {} minutes.",
                impact.window_minutes
            )),
            Some(json!({
                "policyId": incident.policy_id,
                "projectId": incident.project_id,
                "routePolicyId": impact.route_policy_id,
                "tag": incident.tag,
                "textMode": incident.text_mode,
                "code": incident.code,
                "severity": incident.severity,
                "status": incident.status,
                "ownerUserId": incident.owner_user_id,
                "followUpStatus": incident.follow_up_status,
                "syncHitCount": incident.sync_hit_count,
                "escalationStatus": incident.escalation_status,
                "escalatedAt": incident.escalated_at,
                "escalationReason": incident.escalation_reason,
                "latestExportId": incident.latest_export_id,
                "previousExportId": incident.previous_export_id,
                "latestValue": incident.latest_value,
                "previousValue": incident.previous_value,
                "deltaValue": incident.delta_value,
                "deltaRatio": incident.delta_ratio,
                "thresholdValue": incident.threshold_value,
                "remediationRunId": impact.run.id,
                "windowMinutes": impact.window_minutes,
                "completionRateDelta": impact.metrics.completion_rate.delta_value,
                "failureRateDelta": impact.metrics.failure_rate.delta_value,
                "requestArtifactCoverageDelta": impact.metrics.request_artifact_coverage.delta_value,
                "responseArtifactCoverageDelta": impact.metrics.response_artifact_coverage.delta_value,
                "firstTokenLatencyMsAvgDelta": impact.metrics.first_token_latency_ms_avg.delta_value,
                "totalTokensPerSampleDelta": impact.metrics.total_tokens_per_sample.delta_value
            })),
            OffsetDateTime::now_utc(),
        )
        .await?;
    }

    Ok(GatewayAnalysisAnomalyRemediationImpactCaptureView {
        run: to_remediation_run_view(updated_row)?,
        impact,
    })
}

fn build_run_impact(
    generated_at: OffsetDateTime,
    run: GatewayAnalysisAnomalyIncidentRemediationRunView,
    incident: Option<GatewayAnalysisAnomalyIncidentView>,
    project_id: Option<String>,
    route_policy_id: Option<String>,
    anchor_at: OffsetDateTime,
    window_minutes: i32,
    before_started_at: OffsetDateTime,
    before_ended_at: OffsetDateTime,
    before_summary: GatewayAnalysisSummaryView,
    after_started_at: OffsetDateTime,
    after_ended_at: OffsetDateTime,
    after_summary: GatewayAnalysisSummaryView,
) -> GatewayAnalysisAnomalyRemediationRunImpactView {
    GatewayAnalysisAnomalyRemediationRunImpactView {
        generated_at: format_timestamp(generated_at),
        run,
        incident,
        project_id,
        route_policy_id,
        anchor_at: format_timestamp(anchor_at),
        window_minutes,
        before_window: GatewayAnalysisAnomalyRemediationImpactWindowView {
            started_at: format_timestamp(before_started_at),
            ended_at: format_timestamp(before_ended_at),
            summary: before_summary.clone(),
        },
        after_window: GatewayAnalysisAnomalyRemediationImpactWindowView {
            started_at: format_timestamp(after_started_at),
            ended_at: format_timestamp(after_ended_at),
            summary: after_summary.clone(),
        },
        metrics: GatewayAnalysisAnomalyRemediationRunImpactMetricsView {
            completion_rate: build_metric_delta(
                ratio(
                    before_summary.completed_samples,
                    before_summary.total_samples,
                ),
                ratio(after_summary.completed_samples, after_summary.total_samples),
            ),
            failure_rate: build_metric_delta(
                ratio(before_summary.failed_samples, before_summary.total_samples),
                ratio(after_summary.failed_samples, after_summary.total_samples),
            ),
            cancellation_rate: build_metric_delta(
                ratio(
                    before_summary.cancelled_samples,
                    before_summary.total_samples,
                ),
                ratio(after_summary.cancelled_samples, after_summary.total_samples),
            ),
            stream_rate: build_metric_delta(
                ratio(before_summary.stream_samples, before_summary.total_samples),
                ratio(after_summary.stream_samples, after_summary.total_samples),
            ),
            tool_request_rate: build_metric_delta(
                ratio(
                    before_summary.tool_request_samples,
                    before_summary.total_samples,
                ),
                ratio(
                    after_summary.tool_request_samples,
                    after_summary.total_samples,
                ),
            ),
            tool_response_rate: build_metric_delta(
                ratio(
                    before_summary.tool_response_samples,
                    before_summary.total_samples,
                ),
                ratio(
                    after_summary.tool_response_samples,
                    after_summary.total_samples,
                ),
            ),
            request_artifact_coverage: build_metric_delta(
                ratio(
                    before_summary.request_artifact_samples,
                    before_summary.total_samples,
                ),
                ratio(
                    after_summary.request_artifact_samples,
                    after_summary.total_samples,
                ),
            ),
            response_artifact_coverage: build_metric_delta(
                ratio(
                    before_summary.response_artifact_samples,
                    before_summary.total_samples,
                ),
                ratio(
                    after_summary.response_artifact_samples,
                    after_summary.total_samples,
                ),
            ),
            prompt_tokens_per_sample: build_metric_delta(
                per_sample(
                    before_summary.total_prompt_tokens,
                    before_summary.total_samples,
                ),
                per_sample(
                    after_summary.total_prompt_tokens,
                    after_summary.total_samples,
                ),
            ),
            completion_tokens_per_sample: build_metric_delta(
                per_sample(
                    before_summary.total_completion_tokens,
                    before_summary.total_samples,
                ),
                per_sample(
                    after_summary.total_completion_tokens,
                    after_summary.total_samples,
                ),
            ),
            total_tokens_per_sample: build_metric_delta(
                per_sample(before_summary.total_tokens, before_summary.total_samples),
                per_sample(after_summary.total_tokens, after_summary.total_samples),
            ),
            request_text_chars_avg: build_metric_delta(
                before_summary.request_text_chars.avg,
                after_summary.request_text_chars.avg,
            ),
            response_text_chars_avg: build_metric_delta(
                before_summary.response_text_chars.avg,
                after_summary.response_text_chars.avg,
            ),
            first_token_latency_ms_avg: build_metric_delta(
                before_summary.first_token_latency_ms.avg,
                after_summary.first_token_latency_ms.avg,
            ),
            stream_chunk_count_avg: build_metric_delta(
                before_summary.stream_chunk_count.avg,
                after_summary.stream_chunk_count.avg,
            ),
        },
    }
}

pub(super) fn ratio(count: usize, total: usize) -> Option<f64> {
    if total == 0 {
        None
    } else {
        Some(round4(count as f64 / total as f64))
    }
}

fn per_sample(total: i64, sample_count: usize) -> Option<f64> {
    if sample_count == 0 {
        None
    } else {
        Some(round4(total as f64 / sample_count as f64))
    }
}

pub(super) fn build_metric_delta(
    before_value: Option<f64>,
    after_value: Option<f64>,
) -> GatewayAnalysisAnomalyRemediationImpactMetricView {
    let delta_value = match (before_value, after_value) {
        (Some(before), Some(after)) => Some(round4(after - before)),
        _ => None,
    };
    let delta_ratio = match (before_value, after_value) {
        (Some(before), Some(after)) if before != 0.0 => Some(round4((after - before) / before)),
        _ => None,
    };
    GatewayAnalysisAnomalyRemediationImpactMetricView {
        before_value,
        after_value,
        delta_value,
        delta_ratio,
    }
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

pub(super) fn normalize_window_minutes(value: Option<i32>) -> i32 {
    value.unwrap_or(180).clamp(5, 10_080)
}
