//! Minimal real PostgreSQL schema matching the deletion lookup/row contracts.
use sqlx::PgPool;

pub async fn initialize(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        create table gateway_provider_accounts (
          id text primary key, label text not null, service_provider_key text not null default 'fixture',
          service_provider_label text not null default 'Fixture', adapter text not null default 'openai',
          protocol_family text not null default 'openai', protocol_profile text not null default '',
          status text not null default 'active', source_kind text, aggregator_api_mode text,
          web_reverse_access_mode text, source_notes text, execution_mode text not null default 'direct_http',
          endpoint_execution_modes jsonb, payload_inline jsonb default '{}', payload_object_key text,
          storage_mode text not null default 'inline', cooldown_until timestamptz, last_error text,
          failure_count integer not null default 0, last_health_check_at timestamptz,
          created_at timestamptz not null default now(), updated_at timestamptz not null default now()
        );
        create table gateway_provider_credentials (
          id text primary key, provider_account_id text references gateway_provider_accounts(id),
          label text not null, status text not null default 'active', payload_inline jsonb default '{}',
          payload_object_key text, storage_mode text not null default 'inline',
          source_kind text not null default 'folder', source_path text, source_hash text,
          sync_mode text not null default 'folder_sync', sync_state text not null default 'synced',
          sync_error text, cooldown_until timestamptz, last_error text, failure_count integer not null default 0,
          last_health_check_at timestamptz, created_at timestamptz not null default now(),
          updated_at timestamptz not null default now(), archived_at timestamptz
        );
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}
