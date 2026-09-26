use super::*;

#[derive(Debug, Clone, FromRow)]
struct BundleItemExpansionSourceRow {
    provider_account_id: String,
    model_code: String,
    endpoint_kind: String,
    upstream_model: Option<String>,
    platform_tier: String,
    status: String,
    operator_weight: i32,
    routing_priority: i32,
    enabled_for_sale: bool,
    notes: Option<String>,
    capability_enabled: bool,
    protocol_family: String,
    adapter: String,
}

pub(super) fn openai_text_endpoint_family(endpoint_kind: &str) -> &'static [&'static str] {
    match endpoint_kind.trim().to_ascii_lowercase().as_str() {
        "responses" => &["responses", "chat_completions", "messages", "completions"],
        "messages" => &["messages", "responses", "chat_completions", "completions"],
        "completions" => &["completions", "chat_completions", "responses", "messages"],
        "chat_completions" => &["chat_completions", "responses", "messages", "completions"],
        _ => &[],
    }
}

fn supports_openai_bundle_endpoint_pair(protocol_family: &str, adapter: &str) -> bool {
    canonicalize_protocol_family_name(protocol_family) == "openai"
        || matches!(
            canonicalize_adapter_name(adapter).as_str(),
            "openai_compatible" | "freebuff_compatible"
        )
}

async fn ensure_companion_provider_capability_id(
    tx: &mut Transaction<'_, Postgres>,
    row: &BundleItemExpansionSourceRow,
    endpoint_kind: &str,
    now: OffsetDateTime,
) -> Result<String, GatewayError> {
    let existing = sqlx::query(
        r#"
        select id, upstream_model, enabled
        from gateway_provider_capability_catalog
        where provider_account_id = $1
          and model_code = $2
          and endpoint_kind = $3
        order by
          case
            when (
              (upstream_model is null and $4 is null)
              or upstream_model = $4
            ) then 0
            else 1
          end asc,
          created_at asc
        limit 1
        "#,
    )
    .bind(&row.provider_account_id)
    .bind(&row.model_code)
    .bind(endpoint_kind)
    .bind(row.upstream_model.as_deref())
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_db_error)?;

    if let Some(existing) = existing {
        let id = existing.get::<String, _>("id");
        let existing_upstream_model = existing.get::<Option<String>, _>("upstream_model");
        let existing_enabled = existing.get::<bool, _>("enabled");
        if existing_upstream_model.as_deref() != row.upstream_model.as_deref()
            || existing_enabled != row.capability_enabled
        {
            sqlx::query(
                r#"
                update gateway_provider_capability_catalog
                set upstream_model = $2,
                    enabled = $3,
                    updated_at = $4
                where id = $1
                "#,
            )
            .bind(&id)
            .bind(row.upstream_model.as_deref())
            .bind(row.capability_enabled)
            .bind(now)
            .execute(&mut **tx)
            .await
            .map_err(map_db_error)?;
        }
        return Ok(id);
    }

    sqlx::query_scalar::<_, String>(
        r#"
        insert into gateway_provider_capability_catalog (
          id, provider_account_id, model_code, endpoint_kind, upstream_model, enabled, created_at, updated_at
        ) values ($1, $2, $3, $4, $5, $6, $7, $7)
        returning id
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&row.provider_account_id)
    .bind(&row.model_code)
    .bind(endpoint_kind)
    .bind(row.upstream_model.as_deref())
    .bind(row.capability_enabled)
    .bind(now)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_db_error)
}

async fn ensure_companion_platform_access_id(
    tx: &mut Transaction<'_, Postgres>,
    row: &BundleItemExpansionSourceRow,
    provider_capability_id: &str,
    endpoint_kind: &str,
    now: OffsetDateTime,
) -> Result<String, GatewayError> {
    let existing = sqlx::query(
        r#"
        select
          id,
          upstream_model,
          platform_tier,
          status,
          operator_weight,
          routing_priority,
          enabled_for_sale,
          notes
        from gateway_platform_access_catalog
        where provider_capability_id = $1
          and model_code = $2
          and endpoint_kind = $3
        order by
          case
            when (
              (upstream_model is null and $4 is null)
              or upstream_model = $4
            ) then 0
            else 1
          end asc,
          created_at asc
        limit 1
        "#,
    )
    .bind(provider_capability_id)
    .bind(&row.model_code)
    .bind(endpoint_kind)
    .bind(row.upstream_model.as_deref())
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_db_error)?;

    if let Some(existing) = existing {
        let id = existing.get::<String, _>("id");
        let existing_upstream_model = existing.get::<Option<String>, _>("upstream_model");
        let existing_platform_tier = existing.get::<String, _>("platform_tier");
        let existing_status = existing.get::<String, _>("status");
        let existing_operator_weight = existing.get::<i32, _>("operator_weight");
        let existing_routing_priority = existing.get::<i32, _>("routing_priority");
        let existing_enabled_for_sale = existing.get::<bool, _>("enabled_for_sale");
        let existing_notes = existing.get::<Option<String>, _>("notes");
        if existing_upstream_model.as_deref() != row.upstream_model.as_deref()
            || existing_platform_tier != row.platform_tier
            || existing_status != row.status
            || existing_operator_weight != row.operator_weight
            || existing_routing_priority != row.routing_priority
            || existing_enabled_for_sale != row.enabled_for_sale
            || existing_notes.as_deref() != row.notes.as_deref()
        {
            sqlx::query(
                r#"
                update gateway_platform_access_catalog
                set upstream_model = $2,
                    platform_tier = $3,
                    status = $4,
                    operator_weight = $5,
                    routing_priority = $6,
                    enabled_for_sale = $7,
                    notes = $8,
                    updated_at = $9
                where id = $1
                "#,
            )
            .bind(&id)
            .bind(row.upstream_model.as_deref())
            .bind(&row.platform_tier)
            .bind(&row.status)
            .bind(row.operator_weight)
            .bind(row.routing_priority)
            .bind(row.enabled_for_sale)
            .bind(row.notes.as_deref())
            .bind(now)
            .execute(&mut **tx)
            .await
            .map_err(map_db_error)?;
        }
        return Ok(id);
    }

    sqlx::query_scalar::<_, String>(
        r#"
        insert into gateway_platform_access_catalog (
          id, provider_capability_id, model_code, endpoint_kind, upstream_model, platform_tier, status,
          operator_weight, routing_priority, enabled_for_sale, notes, created_at, updated_at
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12)
        returning id
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(provider_capability_id)
    .bind(&row.model_code)
    .bind(endpoint_kind)
    .bind(row.upstream_model.as_deref())
    .bind(&row.platform_tier)
    .bind(&row.status)
    .bind(row.operator_weight)
    .bind(row.routing_priority)
    .bind(row.enabled_for_sale)
    .bind(row.notes.as_deref())
    .bind(now)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_db_error)
}

pub(super) async fn expand_bundle_platform_access_ids_for_openai_compatibility(
    tx: &mut Transaction<'_, Postgres>,
    platform_access_ids: &[String],
) -> Result<Vec<String>, GatewayError> {
    let mut expanded_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for platform_access_id in platform_access_ids {
        if seen.insert(platform_access_id.clone()) {
            expanded_ids.push(platform_access_id.clone());
        }
    }

    if expanded_ids.is_empty() {
        return Ok(expanded_ids);
    }

    let rows = sqlx::query_as::<_, BundleItemExpansionSourceRow>(
        r#"
        select
          pcc.provider_account_id,
          pac.model_code,
          pac.endpoint_kind,
          pac.upstream_model,
          pac.platform_tier,
          pac.status,
          pac.operator_weight,
          pac.routing_priority,
          pac.enabled_for_sale,
          pac.notes,
          pcc.enabled as capability_enabled,
          gpa.protocol_family,
          gpa.adapter
        from gateway_platform_access_catalog pac
        join gateway_provider_capability_catalog pcc
          on pcc.id = pac.provider_capability_id
        join gateway_provider_accounts gpa
          on gpa.id = pcc.provider_account_id
        where pac.id = any($1)
        "#,
    )
    .bind(&expanded_ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(map_db_error)?;

    let now = now_utc();
    for row in rows {
        if !supports_openai_bundle_endpoint_pair(&row.protocol_family, &row.adapter) {
            continue;
        }
        for companion_endpoint_kind in openai_text_endpoint_family(&row.endpoint_kind) {
            if *companion_endpoint_kind == row.endpoint_kind {
                continue;
            }

            let companion_provider_capability_id =
                ensure_companion_provider_capability_id(tx, &row, companion_endpoint_kind, now)
                    .await?;
            let companion_platform_access_id = ensure_companion_platform_access_id(
                tx,
                &row,
                &companion_provider_capability_id,
                companion_endpoint_kind,
                now,
            )
            .await?;
            if seen.insert(companion_platform_access_id.clone()) {
                expanded_ids.push(companion_platform_access_id);
            }
        }
    }

    Ok(expanded_ids)
}
