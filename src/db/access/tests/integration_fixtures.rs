use super::*;
use deadpool_redis::{Config as RedisConfig, Runtime as RedisRuntime};
use sqlx::postgres::PgPoolOptions;

pub(super) async fn access_integration_pool() -> PgPool {
    let database_url = std::env::var("GATEWAY_ACCESS_TEST_DATABASE_URL")
        .expect("set GATEWAY_ACCESS_TEST_DATABASE_URL for ignored access integration tests");
    PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("connect access integration PostgreSQL")
}

pub(super) fn access_integration_redis_pool() -> RedisPool {
    let redis_url = std::env::var("GATEWAY_ACCESS_TEST_REDIS_URL")
        .expect("set GATEWAY_ACCESS_TEST_REDIS_URL for ignored access integration tests");
    RedisConfig::from_url(redis_url)
        .create_pool(Some(RedisRuntime::Tokio1))
        .expect("create access integration Redis pool")
}

pub(super) async fn reset_projection_integration_schema(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        drop table if exists gateway_access_key_bundle_bindings cascade;
        drop table if exists gateway_access_bundle_items cascade;
        drop table if exists gateway_platform_access_catalog cascade;
        drop table if exists gateway_provider_capability_catalog cascade;
        drop table if exists gateway_access_bundles cascade;
        drop table if exists gateway_access_keys cascade;

        create table gateway_access_keys (
          id text primary key,
          resolved_project_id text not null
        );

        create table gateway_access_bundles (
          id text primary key,
          project_id text,
          status text not null
        );

        create table gateway_provider_capability_catalog (
          id text primary key,
          provider_account_id text not null,
          enabled boolean not null
        );

        create table gateway_platform_access_catalog (
          id text primary key,
          provider_capability_id text not null,
          model_code text not null,
          endpoint_kind text not null,
          upstream_model text,
          platform_tier text not null,
          status text not null,
          operator_weight integer not null,
          routing_priority integer not null,
          enabled_for_sale boolean not null,
          notes text,
          created_at timestamptz not null default now(),
          updated_at timestamptz not null default now()
        );

        create table gateway_access_bundle_items (
          bundle_id text not null,
          platform_access_id text not null,
          primary key (bundle_id, platform_access_id)
        );

        create table gateway_access_key_bundle_bindings (
          access_key_id text not null,
          bundle_id text not null,
          primary key (access_key_id, bundle_id)
        );
        "#,
    )
    .execute(pool)
    .await
    .expect("reset projection integration schema");
}

pub(super) async fn reset_rotation_integration_schema(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        drop table if exists gateway_access_key_aggregate_memberships cascade;
        drop table if exists gateway_access_key_balances cascade;
        drop table if exists gateway_access_key_bundle_bindings cascade;
        drop table if exists gateway_access_bundles cascade;
        drop table if exists gateway_access_keys cascade;

        create table gateway_access_keys (
          id text primary key,
          owner_type text not null,
          owner_id text not null,
          resolved_project_id text not null,
          resolved_tenant_id text not null,
          key_kind text not null,
          status text not null,
          public_key_prefix text not null,
          display_name text not null,
          external_key text,
          rotated_from_access_key_id text references gateway_access_keys(id) on delete set null,
          legacy_gateway_api_key_id text unique,
          legacy_user_credential_id text unique,
          expires_at timestamptz,
          last_used_at timestamptz,
          metadata jsonb,
          revoked_at timestamptz,
          revoke_reason text,
          created_at timestamptz not null,
          updated_at timestamptz not null
        );

        create table gateway_access_bundles (
          id text primary key,
          project_id text,
          slug text not null default 'bundle',
          display_name text not null default 'bundle',
          billing_mode text not null default 'time_pass',
          status text not null,
          description text,
          metadata jsonb,
          created_at timestamptz not null default now(),
          updated_at timestamptz not null default now()
        );

        create table gateway_access_key_bundle_bindings (
          access_key_id text not null references gateway_access_keys(id) on delete cascade,
          bundle_id text not null references gateway_access_bundles(id) on delete cascade,
          created_at timestamptz not null,
          primary key (access_key_id, bundle_id)
        );

        create table gateway_access_key_balances (
          access_key_id text primary key references gateway_access_keys(id) on delete cascade,
          balance_mode text not null,
          status text not null,
          unlimited_until timestamptz,
          period_starts_at timestamptz,
          period_ends_at timestamptz,
          total_tokens bigint,
          remaining_tokens bigint,
          total_messages bigint,
          remaining_messages bigint,
          updated_at timestamptz not null
        );

        create table gateway_access_key_aggregate_memberships (
          aggregate_access_key_id text not null references gateway_access_keys(id) on delete cascade,
          member_access_key_id text not null references gateway_access_keys(id) on delete cascade,
          priority integer not null,
          created_at timestamptz not null,
          primary key (aggregate_access_key_id, member_access_key_id)
        );
        "#,
    )
    .execute(pool)
    .await
    .expect("reset rotation integration schema");
}

pub(super) async fn seed_rotation_access_key(
    pool: &PgPool,
    id: &str,
    project_id: &str,
    tenant_id: &str,
    key_kind: &str,
) {
    sqlx::query(
        r#"
        insert into gateway_access_keys (
          id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status,
          public_key_prefix, display_name, created_at, updated_at
        ) values ($1, 'project', $2, $2, $3, $4, 'active', 'gw-project', $1, '1970-01-01T00:00:00Z', '1970-01-01T00:00:00Z')
        "#,
    )
    .bind(id)
    .bind(project_id)
    .bind(tenant_id)
    .bind(key_kind)
    .execute(pool)
    .await
    .expect("seed rotation access key");
}
