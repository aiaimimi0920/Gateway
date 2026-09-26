use super::*;

pub(super) async fn load_provider_candidate_meta_map(
    pool: &PgPool,
    provider_account_ids: &[String],
) -> Result<HashMap<String, ProviderCandidateMetaRow>, GatewayError> {
    if provider_account_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, ProviderCandidateMetaRow>(
        r#"
        select
          id,
          label,
          adapter,
          protocol_family,
          protocol_profile,
          cooldown_until,
          failure_count,
          execution_mode
        from gateway_provider_accounts
        where id = any($1)
          and status = 'active'
        "#,
    )
    .bind(provider_account_ids)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| (row.id.clone(), row))
        .collect::<HashMap<_, _>>())
}

pub(super) async fn build_projection_rows_for_normal_key(
    pool: &PgPool,
    access_key_id: &str,
) -> Result<Vec<ProjectedPlatformAccessRow>, GatewayError> {
    sqlx::query_as::<_, PlatformAccessRow>(
        r#"
        select
          pac.id,
          pac.provider_capability_id,
          pc.provider_account_id,
          pac.model_code,
          pac.endpoint_kind,
          pac.upstream_model,
          pac.platform_tier,
          pac.status,
          pac.operator_weight,
          pac.routing_priority,
          pac.enabled_for_sale,
          pac.notes,
          pac.created_at,
          pac.updated_at
        from gateway_access_keys access_key
        join gateway_access_key_bundle_bindings kb on kb.access_key_id = access_key.id
        join gateway_access_bundles b on b.id = kb.bundle_id
          and b.status = 'active'
          and (b.project_id = access_key.resolved_project_id or b.project_id is null)
        join gateway_access_bundle_items bi on bi.bundle_id = b.id
        join gateway_platform_access_catalog pac on pac.id = bi.platform_access_id
        join gateway_provider_capability_catalog pc on pc.id = pac.provider_capability_id
        where access_key.id = $1
          and pac.status = 'active'
          and pac.enabled_for_sale = true
          and pc.enabled = true
        order by pac.model_code asc, pac.endpoint_kind asc, pac.routing_priority desc, pac.operator_weight desc, pac.id asc
        "#,
    )
    .bind(access_key_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
    .map(|rows| {
        rows.into_iter()
            .map(|row| ProjectedPlatformAccessRow {
                requesting_access_key_id: access_key_id.to_string(),
                source_access_key_id: access_key_id.to_string(),
                platform_access_id: row.id,
                provider_account_id: row.provider_account_id,
                model_code: row.model_code,
                endpoint_kind: row.endpoint_kind,
                upstream_model: row.upstream_model,
                platform_tier: row.platform_tier,
                operator_weight: row.operator_weight,
                routing_priority: row.routing_priority,
            })
            .collect()
    })
}

pub(super) async fn build_projection_rows_for_auto_route_key(
    pool: &PgPool,
    aggregate_access_key_id: &str,
) -> Result<Vec<ProjectedPlatformAccessRow>, GatewayError> {
    let rows = sqlx::query(
        r#"
        select
          member.id as source_access_key_id,
          pac.id as platform_access_id,
          pc.provider_account_id,
          pac.model_code,
          pac.endpoint_kind,
          pac.upstream_model,
          pac.platform_tier,
          pac.operator_weight,
          pac.routing_priority
        from gateway_access_key_aggregate_memberships m
        join gateway_access_keys aggregate on aggregate.id = m.aggregate_access_key_id
        join gateway_access_keys member on member.id = m.member_access_key_id
          and member.resolved_project_id = aggregate.resolved_project_id
          and member.resolved_tenant_id = aggregate.resolved_tenant_id
        join gateway_access_key_bundle_bindings kb on kb.access_key_id = member.id
        join gateway_access_bundles b on b.id = kb.bundle_id
          and b.status = 'active'
          and (b.project_id = aggregate.resolved_project_id or b.project_id is null)
        join gateway_access_bundle_items bi on bi.bundle_id = b.id
        join gateway_platform_access_catalog pac on pac.id = bi.platform_access_id
        join gateway_provider_capability_catalog pc on pc.id = pac.provider_capability_id
        where m.aggregate_access_key_id = $1
          and aggregate.status = 'active'
          and member.status = 'active'
          and (member.expires_at is null or member.expires_at > now())
          and pac.status = 'active'
          and pac.enabled_for_sale = true
          and pc.enabled = true
        order by m.priority asc, pac.model_code asc, pac.endpoint_kind asc, pac.routing_priority desc, pac.operator_weight desc, pac.id asc
        "#,
    )
    .bind(aggregate_access_key_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| ProjectedPlatformAccessRow {
            requesting_access_key_id: aggregate_access_key_id.to_string(),
            source_access_key_id: row.get::<String, _>("source_access_key_id"),
            platform_access_id: row.get::<String, _>("platform_access_id"),
            provider_account_id: row.get::<String, _>("provider_account_id"),
            model_code: row.get::<String, _>("model_code"),
            endpoint_kind: row.get::<String, _>("endpoint_kind"),
            upstream_model: row.get::<Option<String>, _>("upstream_model"),
            platform_tier: row.get::<String, _>("platform_tier"),
            operator_weight: row.get::<i32, _>("operator_weight"),
            routing_priority: row.get::<i32, _>("routing_priority"),
        })
        .collect())
}

pub(super) async fn load_provider_credential_map_for_rows(
    pool: &PgPool,
    provider_account_ids: &[String],
) -> Result<HashMap<String, Vec<super::GatewayProviderCredentialView>>, GatewayError> {
    let credentials =
        list_active_provider_credentials_for_accounts(pool, provider_account_ids).await?;
    let mut result = HashMap::<String, Vec<super::GatewayProviderCredentialView>>::new();
    for credential in credentials {
        result
            .entry(credential.provider_account_id.clone())
            .or_default()
            .push(credential);
    }
    Ok(result)
}

pub(super) async fn load_provider_accounts_with_any_credentials(
    pool: &PgPool,
    provider_account_ids: &[String],
) -> Result<BTreeSet<String>, GatewayError> {
    if provider_account_ids.is_empty() {
        return Ok(BTreeSet::new());
    }

    let rows = sqlx::query_scalar::<_, String>(
        r#"
        select distinct provider_account_id
        from gateway_provider_credentials
        where provider_account_id = any($1)
          and archived_at is null
        "#,
    )
    .bind(provider_account_ids)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    Ok(rows.into_iter().collect())
}
