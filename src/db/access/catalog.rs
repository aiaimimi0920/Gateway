use super::views::{
    to_access_bundle_item_view, to_access_bundle_view, to_access_key_balance_view,
    to_access_key_bundle_binding_view, to_access_key_view, to_aggregate_membership_view,
    to_platform_access_view, to_provider_capability_view,
};
use super::*;

pub async fn list_access_catalog(
    pool: &PgPool,
    api_key_secret: Option<&str>,
) -> Result<GatewayAccessCatalogView, GatewayError> {
    let provider_capabilities = sqlx::query_as::<_, ProviderCapabilityRow>(
        r#"
        select id, provider_account_id, model_code, endpoint_kind, upstream_model, enabled, created_at, updated_at
        from gateway_provider_capability_catalog
        order by provider_account_id asc, model_code asc, endpoint_kind asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_provider_capability_view)
    .collect();

    let platform_access_rows = sqlx::query_as::<_, PlatformAccessRow>(
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
        from gateway_platform_access_catalog pac
        join gateway_provider_capability_catalog pc on pc.id = pac.provider_capability_id
        order by pac.model_code asc, pac.endpoint_kind asc, pac.routing_priority desc, pac.id asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_platform_access_view)
    .collect();

    let bundles = sqlx::query_as::<_, AccessBundleRow>(
        r#"
        select id, project_id, slug, display_name, billing_mode, status, description, metadata, created_at, updated_at
        from gateway_access_bundles
        order by updated_at desc, id desc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_access_bundle_view)
    .collect();

    let bundle_items = sqlx::query_as::<_, AccessBundleItemRow>(
        r#"
        select bundle_id, platform_access_id, created_at
        from gateway_access_bundle_items
        order by bundle_id asc, platform_access_id asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_access_bundle_item_view)
    .collect();

    let access_keys = sqlx::query_as::<_, AccessKeyRow>(
        r#"
        select
          id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status, public_key_prefix,
          display_name, external_key, rotated_from_access_key_id, legacy_gateway_api_key_id, legacy_user_credential_id,
          expires_at, last_used_at, metadata, revoked_at, revoke_reason, created_at, updated_at
        from gateway_access_keys
        order by updated_at desc, id desc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(|row| to_access_key_view(row, api_key_secret))
    .collect::<Result<Vec<_>, _>>()?;

    let key_bundle_bindings = sqlx::query_as::<_, AccessKeyBundleBindingRow>(
        r#"
        select access_key_id, bundle_id, created_at
        from gateway_access_key_bundle_bindings
        order by access_key_id asc, bundle_id asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_access_key_bundle_binding_view)
    .collect::<Vec<_>>();

    let balances = sqlx::query_as::<_, AccessKeyBalanceRow>(
        r#"
        select
          access_key_id, balance_mode, status, unlimited_until, period_starts_at, period_ends_at,
          total_tokens, remaining_tokens, total_messages, remaining_messages, updated_at
        from gateway_access_key_balances
        order by updated_at desc, access_key_id asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_access_key_balance_view)
    .collect();

    let aggregate_memberships = sqlx::query_as::<_, AggregateMembershipRow>(
        r#"
        select aggregate_access_key_id, member_access_key_id, priority, created_at
        from gateway_access_key_aggregate_memberships
        order by aggregate_access_key_id asc, priority asc, member_access_key_id asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_aggregate_membership_view)
    .collect();

    Ok(GatewayAccessCatalogView {
        provider_capabilities,
        platform_access_rows,
        bundles,
        bundle_items,
        access_keys,
        key_bundle_bindings,
        balances,
        aggregate_memberships,
    })
}
