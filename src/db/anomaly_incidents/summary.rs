use super::*;

pub async fn summarize_anomaly_incidents(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyIncidentFilters,
) -> Result<GatewayAnalysisAnomalyIncidentSummaryView, GatewayError> {
    let incidents = list_anomaly_incidents(
        pool,
        &GatewayAnalysisAnomalyIncidentFilters {
            limit: Some(filters.limit.unwrap_or(200).max(200)),
            ..filters.clone()
        },
    )
    .await?;

    let mut by_status = BTreeMap::new();
    let mut by_severity = BTreeMap::new();
    let mut by_code = BTreeMap::new();
    let mut by_follow_up_status = BTreeMap::new();
    let mut by_escalation_status = BTreeMap::new();
    let mut open_incidents = 0;
    let mut acknowledged_incidents = 0;
    let mut resolved_incidents = 0;
    let mut escalated_incidents = 0;

    for incident in &incidents {
        accumulate_key_bucket(&mut by_status, Some(incident.status.as_str()));
        accumulate_key_bucket(&mut by_severity, Some(incident.severity.as_str()));
        accumulate_key_bucket(&mut by_code, Some(incident.code.as_str()));
        accumulate_key_bucket(
            &mut by_follow_up_status,
            Some(incident.follow_up_status.as_str()),
        );
        accumulate_key_bucket(
            &mut by_escalation_status,
            Some(incident.escalation_status.as_str()),
        );
        match incident.status.as_str() {
            "open" => open_incidents += 1,
            "acknowledged" => acknowledged_incidents += 1,
            "resolved" => resolved_incidents += 1,
            _ => {}
        }
        if incident.escalation_status == "escalated" {
            escalated_incidents += 1;
        }
    }

    Ok(GatewayAnalysisAnomalyIncidentSummaryView {
        total_incidents: incidents.len(),
        open_incidents,
        acknowledged_incidents,
        resolved_incidents,
        escalated_incidents,
        by_status: into_key_buckets(by_status),
        by_severity: into_key_buckets(by_severity),
        by_code: into_key_buckets(by_code),
        by_follow_up_status: into_key_buckets(by_follow_up_status),
        by_escalation_status: into_key_buckets(by_escalation_status),
    })
}

fn accumulate_key_bucket(map: &mut BTreeMap<String, usize>, value: Option<&str>) {
    let Some(key) = value.and_then(trimmed_owned_ref) else {
        return;
    };
    *map.entry(key.to_string()).or_insert(0) += 1;
}

fn into_key_buckets(map: BTreeMap<String, usize>) -> Vec<GatewaySummaryBucketKeyView> {
    let mut buckets = map
        .into_iter()
        .map(|(key, count)| GatewaySummaryBucketKeyView { key, count })
        .collect::<Vec<_>>();
    buckets.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
    buckets
}
