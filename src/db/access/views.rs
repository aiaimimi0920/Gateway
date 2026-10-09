use super::keys::build_access_key_token;
use super::*;

pub(super) fn to_provider_capability_view(
    row: ProviderCapabilityRow,
) -> GatewayProviderCapabilityView {
    GatewayProviderCapabilityView {
        id: row.id,
        provider_account_id: row.provider_account_id,
        model_code: row.model_code,
        endpoint_kind: row.endpoint_kind,
        upstream_model: row.upstream_model,
        enabled: row.enabled,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

pub(super) fn to_platform_access_view(row: PlatformAccessRow) -> GatewayPlatformAccessView {
    GatewayPlatformAccessView {
        id: row.id,
        provider_capability_id: row.provider_capability_id,
        provider_account_id: row.provider_account_id,
        model_code: row.model_code,
        endpoint_kind: row.endpoint_kind,
        upstream_model: row.upstream_model,
        platform_tier: row.platform_tier,
        status: row.status,
        operator_weight: row.operator_weight,
        routing_priority: row.routing_priority,
        enabled_for_sale: row.enabled_for_sale,
        notes: row.notes,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

pub(super) fn to_access_bundle_view(row: AccessBundleRow) -> GatewayAccessBundleView {
    GatewayAccessBundleView {
        id: row.id,
        project_id: row.project_id,
        slug: row.slug,
        display_name: row.display_name,
        billing_mode: row.billing_mode,
        status: row.status,
        description: row.description,
        metadata: row.metadata.map(|v| v.0),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

pub(super) fn to_access_bundle_item_view(row: AccessBundleItemRow) -> GatewayAccessBundleItemView {
    GatewayAccessBundleItemView {
        bundle_id: row.bundle_id,
        platform_access_id: row.platform_access_id,
        created_at: format_timestamp(row.created_at),
    }
}

pub(super) fn to_access_key_bundle_binding_view(
    row: AccessKeyBundleBindingRow,
) -> GatewayAccessKeyBundleBindingView {
    GatewayAccessKeyBundleBindingView {
        access_key_id: row.access_key_id,
        bundle_id: row.bundle_id,
        created_at: format_timestamp(row.created_at),
    }
}

pub(super) fn to_access_key_view(
    row: AccessKeyRow,
    api_key_secret: Option<&str>,
) -> Result<GatewayAccessKeyView, GatewayError> {
    let token = build_access_key_token(&row, api_key_secret)?;
    Ok(GatewayAccessKeyView {
        id: row.id,
        owner_type: row.owner_type,
        owner_id: row.owner_id,
        resolved_project_id: row.resolved_project_id,
        resolved_tenant_id: row.resolved_tenant_id,
        key_kind: row.key_kind,
        status: row.status,
        public_key_prefix: row.public_key_prefix,
        display_name: row.display_name,
        token,
        external_key: row.external_key,
        rotated_from_access_key_id: row.rotated_from_access_key_id,
        legacy_gateway_api_key_id: row.legacy_gateway_api_key_id,
        legacy_user_credential_id: row.legacy_user_credential_id,
        expires_at: row.expires_at.map(format_timestamp),
        last_used_at: row.last_used_at.map(format_timestamp),
        metadata: row.metadata.map(|value| value.0),
        revoked_at: row.revoked_at.map(format_timestamp),
        revoke_reason: row.revoke_reason,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    })
}

pub(super) fn to_access_key_balance_view(row: AccessKeyBalanceRow) -> GatewayAccessKeyBalanceView {
    GatewayAccessKeyBalanceView {
        access_key_id: row.access_key_id,
        balance_mode: row.balance_mode,
        status: row.status,
        unlimited_until: row.unlimited_until.map(format_timestamp),
        period_starts_at: row.period_starts_at.map(format_timestamp),
        period_ends_at: row.period_ends_at.map(format_timestamp),
        total_tokens: row.total_tokens,
        remaining_tokens: row.remaining_tokens,
        total_messages: row.total_messages,
        remaining_messages: row.remaining_messages,
        cash: None,
        updated_at: format_timestamp(row.updated_at),
    }
}

pub(super) fn to_aggregate_membership_view(
    row: AggregateMembershipRow,
) -> GatewayAccessKeyAggregateMembershipView {
    GatewayAccessKeyAggregateMembershipView {
        aggregate_access_key_id: row.aggregate_access_key_id,
        member_access_key_id: row.member_access_key_id,
        priority: row.priority,
        created_at: format_timestamp(row.created_at),
    }
}
