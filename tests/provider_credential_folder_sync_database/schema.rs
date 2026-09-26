//! Reuse deletion row contracts, adding the columns required by actual mutations.
#[path = "../provider_management_deletion/schema.rs"]
mod deletion;

pub async fn initialize(pool: &sqlx::PgPool) {
    deletion::initialize(pool).await;
    sqlx::raw_sql(
        "alter table gateway_provider_accounts add column payload_content_type text;
         alter table gateway_provider_credentials add column payload_content_type text;",
    )
    .execute(pool)
    .await
    .unwrap();
}
