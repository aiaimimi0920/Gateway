use super::*;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GatewayRequestAnalysisProfilePayload {
    request_tool_count: Option<i64>,
    request_historical_tool_call_count: Option<i64>,
    has_system_prompt: Option<bool>,
    has_reasoning: Option<bool>,
    has_metadata: Option<bool>,
    has_explicit_session_key: Option<bool>,
    has_previous_response: Option<bool>,
    response_tool_call_count: Option<i64>,
    request_text_chars: Option<i64>,
    response_text_chars: Option<i64>,
    first_token_latency_ms: Option<i64>,
    stream_chunk_count: Option<i64>,
}

pub async fn list_analysis_samples(
    pool: &PgPool,
    filters: &RequestAuditFilters,
) -> Result<Vec<GatewayAnalysisSampleView>, GatewayError> {
    let rows = list_request_audits(pool, filters).await?;
    Ok(rows.into_iter().map(to_analysis_sample_view).collect())
}

pub async fn summarize_analysis(
    pool: &PgPool,
    filters: &RequestAuditFilters,
) -> Result<GatewayAnalysisSummaryView, GatewayError> {
    let rows = list_analysis_samples(pool, filters).await?;

    let mut by_protocol_family = BTreeMap::new();
    let mut by_endpoint_kind = BTreeMap::new();
    let mut by_resolved_model = BTreeMap::new();
    let mut by_provider_account = BTreeMap::new();
    let mut by_status = BTreeMap::new();

    let mut completed_samples = 0;
    let mut failed_samples = 0;
    let mut cancelled_samples = 0;
    let mut stream_samples = 0;
    let mut tool_request_samples = 0;
    let mut tool_response_samples = 0;
    let mut system_prompt_samples = 0;
    let mut reasoning_samples = 0;
    let mut metadata_samples = 0;
    let mut explicit_session_samples = 0;
    let mut previous_response_samples = 0;
    let mut request_artifact_samples = 0;
    let mut response_artifact_samples = 0;
    let mut total_prompt_tokens = 0_i64;
    let mut total_completion_tokens = 0_i64;
    let mut total_tokens = 0_i64;
    let mut total_cache_creation_input_tokens = 0_i64;
    let mut total_cache_read_input_tokens = 0_i64;

    let mut request_text_chars = Vec::new();
    let mut response_text_chars = Vec::new();
    let mut first_token_latency_ms = Vec::new();
    let mut stream_chunk_count = Vec::new();

    for row in &rows {
        accumulate_bucket(&mut by_protocol_family, Some(row.protocol_family.as_str()));
        accumulate_bucket(&mut by_endpoint_kind, Some(row.endpoint_kind.as_str()));
        accumulate_bucket(&mut by_resolved_model, row.resolved_model.as_deref());
        accumulate_bucket(&mut by_provider_account, row.provider_account_id.as_deref());
        accumulate_bucket(&mut by_status, Some(row.status.as_str()));

        match row.status.as_str() {
            "completed" => completed_samples += 1,
            "failed" => failed_samples += 1,
            "cancelled" => cancelled_samples += 1,
            _ => {}
        }

        if row.stream {
            stream_samples += 1;
        }

        let analysis_profile = parse_analysis_profile(row.analysis_profile.as_ref());
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.request_tool_count.unwrap_or(0) > 0)
            || analysis_profile
                .as_ref()
                .is_some_and(|profile| profile.request_historical_tool_call_count.unwrap_or(0) > 0)
        {
            tool_request_samples += 1;
        }
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.response_tool_call_count.unwrap_or(0) > 0)
        {
            tool_response_samples += 1;
        }
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.has_system_prompt.unwrap_or(false))
        {
            system_prompt_samples += 1;
        }
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.has_reasoning.unwrap_or(false))
        {
            reasoning_samples += 1;
        }
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.has_metadata.unwrap_or(false))
        {
            metadata_samples += 1;
        }
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.has_explicit_session_key.unwrap_or(false))
        {
            explicit_session_samples += 1;
        }
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.has_previous_response.unwrap_or(false))
        {
            previous_response_samples += 1;
        }
        if row.request_artifact_object_key.is_some() {
            request_artifact_samples += 1;
        }
        if row.response_artifact_object_key.is_some() {
            response_artifact_samples += 1;
        }

        total_prompt_tokens += i64::from(row.prompt_tokens.unwrap_or(0));
        total_completion_tokens += i64::from(row.completion_tokens.unwrap_or(0));
        total_tokens += i64::from(row.total_tokens.unwrap_or(0));
        total_cache_creation_input_tokens +=
            i64::from(row.cache_creation_input_tokens.unwrap_or(0));
        total_cache_read_input_tokens += i64::from(row.cache_read_input_tokens.unwrap_or(0));

        request_text_chars.push(
            analysis_profile
                .as_ref()
                .and_then(|profile| profile.request_text_chars),
        );
        response_text_chars.push(
            analysis_profile
                .as_ref()
                .and_then(|profile| profile.response_text_chars),
        );
        first_token_latency_ms.push(
            analysis_profile
                .as_ref()
                .and_then(|profile| profile.first_token_latency_ms),
        );
        stream_chunk_count.push(
            analysis_profile
                .as_ref()
                .and_then(|profile| profile.stream_chunk_count),
        );
    }

    Ok(GatewayAnalysisSummaryView {
        total_samples: rows.len(),
        completed_samples,
        failed_samples,
        cancelled_samples,
        stream_samples,
        tool_request_samples,
        tool_response_samples,
        system_prompt_samples,
        reasoning_samples,
        metadata_samples,
        explicit_session_samples,
        previous_response_samples,
        request_artifact_samples,
        response_artifact_samples,
        total_prompt_tokens,
        total_completion_tokens,
        total_tokens,
        total_cache_creation_input_tokens,
        total_cache_read_input_tokens,
        request_text_chars: build_distribution(&request_text_chars),
        response_text_chars: build_distribution(&response_text_chars),
        first_token_latency_ms: build_distribution(&first_token_latency_ms),
        stream_chunk_count: build_distribution(&stream_chunk_count),
        by_protocol_family: into_summary_buckets(by_protocol_family),
        by_endpoint_kind: into_summary_buckets(by_endpoint_kind),
        by_resolved_model: into_summary_buckets(by_resolved_model),
        by_provider_account: into_summary_buckets(by_provider_account),
        by_status: into_summary_buckets(by_status),
    })
}

fn to_analysis_sample_view(row: GatewayRequestAuditView) -> GatewayAnalysisSampleView {
    GatewayAnalysisSampleView {
        request_audit_id: row.id,
        response_id: row.response_id,
        project_id: row.project_id,
        route_policy_id: row.route_policy_id,
        session_id: row.session_id,
        provider_account_id: row.provider_account_id,
        protocol_family: row.protocol_family,
        endpoint_kind: row.endpoint_kind,
        requested_model: row.requested_model,
        resolved_model: row.resolved_model,
        status: row.status,
        stream: row.stream,
        created_at: row.created_at,
        completed_at: row.completed_at,
        prompt_tokens: row.prompt_tokens,
        completion_tokens: row.completion_tokens,
        total_tokens: row.total_tokens,
        cache_creation_input_tokens: row.cache_creation_input_tokens,
        cache_read_input_tokens: row.cache_read_input_tokens,
        analysis_profile: row.analysis_profile,
        request_artifact_object_key: row.request_artifact_object_key,
        response_artifact_object_key: row.response_artifact_object_key,
        route_trace: row.route_trace,
    }
}

fn parse_analysis_profile(value: Option<&Value>) -> Option<GatewayRequestAnalysisProfilePayload> {
    value.and_then(|payload| serde_json::from_value(payload.clone()).ok())
}
