//! Provider capability and platform-access row loading for model tiering.

use crate::db::map_db_error;
use crate::error::GatewayError;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub(super) struct ProviderCapabilityTieringRow {
    pub(super) id: String,
    pub(super) provider_account_id: String,
    pub(super) model_code: String,
    pub(super) endpoint_kind: String,
    pub(super) upstream_model: Option<String>,
    pub(super) enabled: bool,
}

#[derive(Debug, Clone, FromRow)]
pub(super) struct PlatformAccessTieringRow {
    pub(super) id: String,
    pub(super) provider_capability_id: String,
    pub(super) model_code: String,
    pub(super) endpoint_kind: String,
    pub(super) upstream_model: Option<String>,
    pub(super) platform_tier: String,
    pub(super) status: String,
    pub(super) operator_weight: i32,
    pub(super) routing_priority: i32,
    pub(super) enabled_for_sale: bool,
    pub(super) notes: Option<String>,
}

pub(super) async fn load_provider_capability_tiering_rows(
    pool: &sqlx::PgPool,
    provider_account_id: &str,
) -> Result<Vec<ProviderCapabilityTieringRow>, GatewayError> {
    sqlx::query_as::<_, ProviderCapabilityTieringRow>(
        r#"
        select
          id,
          provider_account_id,
          model_code,
          endpoint_kind,
          upstream_model,
          enabled
        from gateway_provider_capability_catalog
        where provider_account_id = $1
        order by enabled desc, updated_at desc, model_code asc, endpoint_kind asc
        "#,
    )
    .bind(provider_account_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
}

pub(super) async fn load_platform_access_tiering_rows(
    pool: &sqlx::PgPool,
    provider_account_id: &str,
) -> Result<Vec<PlatformAccessTieringRow>, GatewayError> {
    sqlx::query_as::<_, PlatformAccessTieringRow>(
        r#"
        select
          pac.id,
          pac.provider_capability_id,
          pac.model_code,
          pac.endpoint_kind,
          pac.upstream_model,
          pac.platform_tier,
          pac.status,
          pac.operator_weight,
          pac.routing_priority,
          pac.enabled_for_sale,
          pac.notes
        from gateway_platform_access_catalog pac
        join gateway_provider_capability_catalog pc on pc.id = pac.provider_capability_id
        where pc.provider_account_id = $1
        order by pac.enabled_for_sale desc, pac.updated_at desc, pac.model_code asc, pac.endpoint_kind asc
        "#,
    )
    .bind(provider_account_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
}
