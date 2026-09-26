use super::dataset::build_analysis_dataset_jsonl;
use super::manifest::{build_analysis_export_file_view, build_manifest_artifacts};
use super::messages::{
    coerce_message_export, coerce_request_tool_names, coerce_response_tool_names,
};
use super::normalization::{
    normalize_export_label, normalize_export_tags, normalize_max_text_chars, normalize_text_mode,
    parse_optional_rfc3339,
};
use super::record_views::to_persisted_analysis_export_view;
use super::text::apply_text_mode;
use super::*;
use uuid::Uuid;

pub async fn export_analysis_rows(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    text_mode: Option<&str>,
    max_text_chars: Option<usize>,
) -> Result<GatewayAnalysisExportView, GatewayError> {
    let normalized_text_mode = normalize_text_mode(text_mode);
    let normalized_max_text_chars = normalize_max_text_chars(max_text_chars);
    let export_filters = normalized_export_request_filters(filters);
    let samples = list_analysis_samples(pool, &export_filters).await?;
    let storage = gateway_object_storage()?;
    let mut rows = Vec::with_capacity(samples.len());

    for sample in samples {
        let request_artifact = match sample.request_artifact_object_key.as_deref() {
            Some(object_key) => storage.read_json(object_key).await.ok(),
            None => None,
        };
        let response_artifact = match sample.response_artifact_object_key.as_deref() {
            Some(object_key) => storage.read_json(object_key).await.ok(),
            None => None,
        };
        rows.push(build_export_row(
            &sample,
            request_artifact.as_ref(),
            response_artifact.as_ref(),
            normalized_text_mode,
            normalized_max_text_chars,
        ));
    }

    Ok(GatewayAnalysisExportView {
        text_mode: normalized_text_mode.to_string(),
        max_text_chars: normalized_max_text_chars,
        sample_count: rows.len(),
        request_artifact_count: rows
            .iter()
            .filter(|row| row.request_artifact_available)
            .count(),
        response_artifact_count: rows
            .iter()
            .filter(|row| row.response_artifact_available)
            .count(),
        rows,
    })
}

pub async fn persist_analysis_export(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    text_mode: Option<&str>,
    max_text_chars: Option<usize>,
    input: PersistGatewayAnalysisExportInput,
) -> Result<GatewayPersistedAnalysisExportView, GatewayError> {
    let normalized_text_mode = normalize_text_mode(text_mode);
    let normalized_max_text_chars = normalize_max_text_chars(max_text_chars);
    let normalized_filters = normalized_export_request_filters(filters);
    let export_view = export_analysis_rows(
        pool,
        &normalized_filters,
        Some(normalized_text_mode),
        Some(normalized_max_text_chars),
    )
    .await?;
    let export_id = Uuid::new_v4().to_string();
    let created_at = OffsetDateTime::now_utc();
    let label = normalize_export_label(input.label.as_deref())?;
    let tags = normalize_export_tags(input.tags.as_ref())?;
    let retention_expires_at =
        parse_optional_rfc3339(input.retention_expires_at.as_deref(), "retentionExpiresAt")?;
    let filter_view = build_export_filter_view(
        &normalized_filters,
        normalized_text_mode,
        normalized_max_text_chars,
    );
    let object_prefix = build_gateway_analysis_export_prefix(&export_id);
    let dataset_object_key = build_gateway_analysis_export_dataset_object_key(&export_id);
    let dataset_body = build_analysis_dataset_jsonl(&export_view.rows)?;
    let dataset_file = build_analysis_export_file_view(
        "dataset_jsonl",
        dataset_object_key.clone(),
        "application/x-ndjson",
        &dataset_body,
        Some(export_view.rows.len()),
    );
    let manifest_object_key = build_gateway_analysis_export_manifest_object_key(&export_id);
    let manifest_artifacts = build_manifest_artifacts(
        &export_id,
        label.clone(),
        tags.clone(),
        format_timestamp(created_at),
        retention_expires_at.map(format_timestamp),
        filter_view.clone(),
        export_view.sample_count,
        export_view.request_artifact_count,
        export_view.response_artifact_count,
        dataset_file,
        manifest_object_key.clone(),
    )?;
    let storage = gateway_object_storage()?;
    storage
        .put_bytes(&dataset_object_key, dataset_body, "application/x-ndjson")
        .await?;
    storage
        .put_bytes(
            &manifest_object_key,
            manifest_artifacts.manifest_body.clone(),
            "application/json",
        )
        .await?;

    sqlx::query(
        r#"
        insert into gateway_analysis_exports (
          id,
          project_id,
          label,
          tags,
          status,
          text_mode,
          max_text_chars,
          filters,
          object_prefix,
          manifest_object_key,
          dataset_object_key,
          sample_count,
          request_artifact_count,
          response_artifact_count,
          retention_expires_at,
          cleaned_up_at,
          last_cleanup_error,
          created_at,
          updated_at
        ) values (
          $1, $2, $3, $4, 'active', $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, null, null, $15, $15
        )
        "#,
    )
    .bind(&export_id)
    .bind(filter_view.project_id.as_deref())
    .bind(label.as_deref())
    .bind(Json(tags))
    .bind(normalized_text_mode)
    .bind(i32::try_from(normalized_max_text_chars).unwrap_or(i32::MAX))
    .bind(Json(serde_json::to_value(&filter_view).map_err(|error| {
        GatewayError::server_error(format!("serialize analysis export filters: {error}"))
    })?))
    .bind(&object_prefix)
    .bind(&manifest_object_key)
    .bind(&dataset_object_key)
    .bind(i32::try_from(export_view.sample_count).unwrap_or(i32::MAX))
    .bind(i32::try_from(export_view.request_artifact_count).unwrap_or(i32::MAX))
    .bind(i32::try_from(export_view.response_artifact_count).unwrap_or(i32::MAX))
    .bind(retention_expires_at)
    .bind(created_at)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let row = sqlx::query_as::<_, GatewayAnalysisExportRow>(
        r#"
        select
          id, project_id, label, tags, status, text_mode, max_text_chars, filters,
          object_prefix, manifest_object_key, dataset_object_key, sample_count,
          request_artifact_count, response_artifact_count, retention_expires_at,
          cleaned_up_at, last_cleanup_error, created_at, updated_at
        from gateway_analysis_exports
        where id = $1
        limit 1
        "#,
    )
    .bind(&export_id)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    to_persisted_analysis_export_view(row, Some(manifest_artifacts.manifest))
}

fn normalized_export_request_filters(filters: &RequestAuditFilters) -> RequestAuditFilters {
    RequestAuditFilters {
        limit: Some(
            filters
                .limit
                .unwrap_or(DEFAULT_EXPORT_LIMIT)
                .clamp(1, MAX_EXPORT_LIMIT),
        ),
        ..filters.clone()
    }
}

fn build_export_filter_view(
    filters: &RequestAuditFilters,
    text_mode: &str,
    max_text_chars: usize,
) -> GatewayAnalysisExportFilterView {
    GatewayAnalysisExportFilterView {
        project_id: filters.project_id.clone(),
        route_policy_id: filters.route_policy_id.clone(),
        provider_account_id: filters.provider_account_id.clone(),
        session_id: filters.session_id.clone(),
        api_key_id: filters.api_key_id.clone(),
        response_id: filters.response_id.clone(),
        protocol_family: filters.protocol_family.clone(),
        status: filters.status.clone(),
        endpoint_kind: filters.endpoint_kind.clone(),
        stream: filters.stream,
        error_code: filters.error_code.clone(),
        fallback_eligible: filters.fallback_eligible,
        created_from: filters.created_from.clone(),
        created_to: filters.created_to.clone(),
        artifact_available: filters.artifact_available,
        limit: filters.limit.unwrap_or(DEFAULT_EXPORT_LIMIT).clamp(1, 1000),
        text_mode: text_mode.to_string(),
        max_text_chars,
    }
}

fn build_export_row(
    sample: &GatewayAnalysisSampleView,
    request_artifact: Option<&Value>,
    response_artifact: Option<&Value>,
    text_mode: &str,
    max_text_chars: usize,
) -> GatewayAnalysisExportRowView {
    let mut request_messages = request_artifact
        .and_then(|artifact| {
            artifact
                .get("canonicalRequest")
                .and_then(|value| value.get("messages"))
                .and_then(Value::as_array)
                .cloned()
        })
        .unwrap_or_default()
        .into_iter()
        .filter_map(|message| coerce_message_export(&message))
        .collect::<Vec<_>>();
    let request_text_source = request_messages
        .iter()
        .map(|message| message.text.as_str())
        .filter(|item| !item.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    let response_text_source = response_artifact
        .and_then(|artifact| artifact.get("result"))
        .and_then(|value| value.get("text"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let request_text = apply_text_mode(&request_text_source, text_mode, max_text_chars);
    let response_text = apply_text_mode(&response_text_source, text_mode, max_text_chars);

    // Structured messages must obey the same text policy as flattened fields.
    for message in &mut request_messages {
        message.text = apply_text_mode(&message.text, text_mode, max_text_chars)
            .text
            .unwrap_or_default();
    }

    GatewayAnalysisExportRowView {
        request_audit_id: sample.request_audit_id.clone(),
        response_id: sample.response_id.clone(),
        project_id: sample.project_id.clone(),
        session_id: sample.session_id.clone(),
        provider_account_id: sample.provider_account_id.clone(),
        protocol_family: sample.protocol_family.clone(),
        endpoint_kind: sample.endpoint_kind.clone(),
        requested_model: sample.requested_model.clone(),
        resolved_model: sample.resolved_model.clone(),
        status: sample.status.clone(),
        stream: sample.stream,
        created_at: sample.created_at.clone(),
        completed_at: sample.completed_at.clone(),
        prompt_tokens: sample.prompt_tokens,
        completion_tokens: sample.completion_tokens,
        total_tokens: sample.total_tokens,
        request_artifact_available: sample.request_artifact_object_key.is_some(),
        response_artifact_available: sample.response_artifact_object_key.is_some(),
        analysis_profile: sample.analysis_profile.clone(),
        route_trace: sample.route_trace.clone(),
        request_text: request_text.text,
        response_text: response_text.text,
        request_text_truncated: request_text.truncated,
        response_text_truncated: response_text.truncated,
        request_messages,
        request_tool_names: coerce_request_tool_names(request_artifact),
        response_tool_names: coerce_response_tool_names(response_artifact),
    }
}

#[cfg(test)]
mod text_mode_tests;
