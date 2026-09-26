use super::*;

pub(super) async fn fetch_active_provider_rows(
    pool: &PgPool,
) -> Result<Vec<GatewayProviderRouteRow>, GatewayError> {
    recover_expired_cooling_provider_accounts(pool).await?;
    sqlx::query_as::<_, GatewayProviderRouteRow>(
        r#"
        select
          id,
          label,
          adapter,
          protocol_family,
          protocol_profile,
          cooldown_until,
          failure_count,
          execution_mode,
          endpoint_execution_modes,
          payload_inline,
          payload_object_key
        from gateway_provider_accounts
        where status = 'active'
          and (cooldown_until is null or cooldown_until <= now())
        order by created_at asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
}

pub(super) async fn provider_payload_value_from_row(
    row: &GatewayProviderRouteRow,
) -> Result<Option<Value>, GatewayError> {
    let payload_value = match (&row.payload_inline, row.payload_object_key.as_deref()) {
        (Some(payload_inline), _) => payload_inline.0.clone(),
        (None, Some(object_key)) => gateway_object_storage()?.read_json(object_key).await?,
        (None, None) => return Ok(None),
    };
    Ok(Some(payload_value))
}

pub(super) fn provider_payload_from_value(
    row: &GatewayProviderRouteRow,
    payload_value: Value,
) -> Result<Option<ProviderAccountPayload>, GatewayError> {
    let mut payload: ProviderAccountPayload = match serde_json::from_value(payload_value) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!(
                provider_account_id = %row.id,
                error = %error,
                "skip malformed provider payload while building gateway model and route catalog"
            );
            return Ok(None);
        }
    };
    payload.adapter = canonicalize_adapter_name(&row.adapter);
    payload.execution_mode = parse_execution_mode(&row.execution_mode).or(payload.execution_mode);
    if let Some(endpoint_execution_modes) = &row.endpoint_execution_modes {
        payload.endpoint_execution_modes = Some(
            serde_json::from_value(endpoint_execution_modes.0.clone()).map_err(|error| {
                GatewayError::server_error(format!(
                    "deserialize endpoint execution modes for {}: {error}",
                    row.id
                ))
            })?,
        );
    }
    Ok(Some(payload))
}

pub(super) async fn build_route_candidates_for_provider(
    provider_row: &GatewayProviderRouteRow,
    credentials: Option<&Vec<super::GatewayProviderCredentialView>>,
    endpoint_kind: EndpointKind,
    model_alias: Option<String>,
    upstream_model: Option<String>,
    priority: i32,
    weight: i32,
) -> Result<Vec<RouteCandidate>, GatewayError> {
    let Some(account_payload) = provider_payload_value_from_row(provider_row).await? else {
        return Ok(Vec::new());
    };
    let mut candidates = Vec::new();
    if let Some(credentials) = credentials {
        for credential in credentials {
            if !credential_payload_supports_model(
                &credential.payload,
                model_alias.as_deref(),
                upstream_model.as_deref(),
            ) {
                continue;
            }
            let merged_payload = merge_provider_account_and_credential_payloads(
                &account_payload,
                &credential.payload,
            );
            let supported_protocol_families = resolve_supported_wire_protocol_families_for_model(
                Some(&merged_payload),
                model_alias.as_deref(),
                upstream_model.as_deref(),
                &provider_row.adapter,
                &provider_row.protocol_family,
            );
            let Some(mut payload) = provider_payload_from_value(provider_row, merged_payload)?
            else {
                continue;
            };
            payload.credential_id = Some(credential.id.clone());
            candidates.push(RouteCandidate {
                provider_account_id: provider_row.id.clone(),
                provider_credential_id: Some(credential.id.clone()),
                label: format!("{} / {}", provider_row.label, credential.label),
                payload: payload.clone(),
                protocol_family: canonicalize_protocol_family_name(&provider_row.protocol_family),
                protocol_profile: provider_row.protocol_profile.clone(),
                supported_protocol_families,
                adapter: canonicalize_adapter_name(&provider_row.adapter),
                model_alias: model_alias.clone(),
                upstream_model: upstream_model.clone(),
                resolved_execution_mode: payload.resolve_execution_mode(endpoint_kind),
                priority,
                weight,
                failure_count: credential.failure_count.max(0) as u32,
                cooldown_until: credential.cooldown_until.clone(),
                routing_score: None,
                routing_health_weight: None,
                routing_capacity_weight: None,
                routing_degraded: None,
                routing_breaker_open: None,
                routing_degradation_reasons: Vec::new(),
            });
        }
        return Ok(candidates);
    }

    let supported_protocol_families = resolve_supported_wire_protocol_families_for_model(
        Some(&account_payload),
        model_alias.as_deref(),
        upstream_model.as_deref(),
        &provider_row.adapter,
        &provider_row.protocol_family,
    );
    let Some(payload) = provider_payload_from_value(provider_row, account_payload)? else {
        return Ok(Vec::new());
    };
    candidates.push(RouteCandidate {
        provider_account_id: provider_row.id.clone(),
        provider_credential_id: None,
        label: provider_row.label.clone(),
        payload: payload.clone(),
        protocol_family: canonicalize_protocol_family_name(&provider_row.protocol_family),
        protocol_profile: provider_row.protocol_profile.clone(),
        supported_protocol_families,
        adapter: canonicalize_adapter_name(&provider_row.adapter),
        model_alias,
        upstream_model,
        resolved_execution_mode: payload.resolve_execution_mode(endpoint_kind),
        priority,
        weight,
        failure_count: provider_row.failure_count.max(0) as u32,
        cooldown_until: provider_row.cooldown_until.map(format_timestamp),
        routing_score: None,
        routing_health_weight: None,
        routing_capacity_weight: None,
        routing_degraded: None,
        routing_breaker_open: None,
        routing_degradation_reasons: Vec::new(),
    });
    Ok(candidates)
}

fn parse_execution_mode(value: &str) -> Option<ProviderExecutionMode> {
    match value.trim().to_lowercase().as_str() {
        "browser_backed" => Some(ProviderExecutionMode::BrowserBacked),
        "direct_http" => Some(ProviderExecutionMode::DirectHttp),
        _ => None,
    }
}
