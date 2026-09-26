//! Running-request pressure and runtime concurrency projections.

use super::runtime_state::read_breaker_open_map;
use super::*;
use redis::AsyncCommands;

pub async fn get_runtime_pressure(
    pool: &PgPool,
    redis_pool: &Pool,
    concurrency_snapshots: &HashMap<String, ConcurrencySnapshot>,
    runtime_providers: &[GatewayRuntimeProviderIdentity],
    filters: &GatewayRuntimePressureFilters,
) -> Result<GatewayRuntimePressureView, GatewayError> {
    let limit = filters.limit.unwrap_or(100).clamp(1, 500) as i64;
    let running_rows = sqlx::query_as::<_, RunningAuditRow>(
        r#"
        select project_id, provider_account_id
        from gateway_request_audits
        where status = 'running'
          and ($1::text is null or project_id = $1)
          and ($2::text is null or provider_account_id = $2)
        order by created_at desc
        limit $3
        "#,
    )
    .bind(filters.project_id.as_deref())
    .bind(filters.provider_account_id.as_deref())
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    let mut project_ids = running_rows
        .iter()
        .map(|row| row.project_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(project_id) = filters.project_id.as_ref() {
        project_ids.insert(project_id.clone());
    }
    let running_provider_ids = running_rows
        .iter()
        .filter_map(|row| row.provider_account_id.clone())
        .collect::<std::collections::BTreeSet<_>>();

    let project_rows = if project_ids.is_empty() {
        Vec::new()
    } else {
        let ids = project_ids.into_iter().collect::<Vec<_>>();
        let placeholders = (1..=ids.len())
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql =
            format!("select id, display_name from gateway_projects where id in ({placeholders})");
        let mut query = sqlx::query_as::<_, GatewayProjectDisplayRow>(&sql);
        for id in &ids {
            query = query.bind(id);
        }
        query.fetch_all(pool).await.map_err(map_db_error)?
    };
    // Every provider the runtime can serve is reported, not only the ones holding
    // a request right now: an idle provider's concurrency is a known zero, and a
    // card left out of this list has to render `—` as if the number were
    // unavailable. A provider filter narrows the list back to one provider.
    let provider_rows = {
        let wanted = filters.provider_account_id.as_deref();
        let mut rows = list_provider_accounts(pool)
            .await?
            .into_iter()
            .filter(|provider| wanted.is_none_or(|id| provider.id == id))
            .map(|provider| GatewayRuntimeProviderIdentity {
                id: provider.id,
                label: provider.label,
                status: provider.status,
                adapter: provider.adapter,
                protocol_family: provider.protocol_family,
                supported_models: Vec::new(),
            })
            .collect::<Vec<_>>();
        // A route-document provider has no `gateway_provider_accounts` row, so
        // the query above cannot name it even though its id is on the audits.
        // Dropping it here would leave a standalone deployment with no
        // per-provider concurrency at all.
        let described = rows
            .iter()
            .map(|row| row.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        rows.extend(
            runtime_providers
                .iter()
                .filter(|provider| {
                    !described.contains(&provider.id) && wanted.is_none_or(|id| provider.id == id)
                })
                .cloned(),
        );
        // A request can still name a provider neither source knows any more, such
        // as one deleted while it was serving traffic. Keeping it means its
        // in-flight requests stay in the totals instead of disappearing.
        let known = rows
            .iter()
            .map(|row| row.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        for provider_account_id in &running_provider_ids {
            if known.contains(provider_account_id)
                || wanted.is_some_and(|id| id != provider_account_id.as_str())
            {
                continue;
            }
            rows.push(GatewayRuntimeProviderIdentity {
                id: provider_account_id.clone(),
                label: provider_account_id.clone(),
                status: "unknown".to_string(),
                adapter: String::new(),
                protocol_family: String::new(),
                supported_models: Vec::new(),
            });
        }
        rows
    };

    let mut project_counts = HashMap::<String, usize>::new();
    let mut provider_counts = HashMap::<String, usize>::new();
    for row in &running_rows {
        *project_counts
            .entry(row.project_id.clone())
            .or_insert(0usize) += 1;
        if let Some(provider_account_id) = row.provider_account_id.as_ref() {
            *provider_counts
                .entry(provider_account_id.clone())
                .or_insert(0usize) += 1;
        }
    }

    let mut projects = Vec::with_capacity(project_rows.len());
    for project in project_rows {
        let running_request_count = project_counts.get(&project.id).copied().unwrap_or(0);
        let active_concurrency =
            read_redis_int(redis_pool, &legacy_project_concurrency_key(&project.id))
                .await
                .unwrap_or(running_request_count);
        projects.push(GatewayProjectPressureView {
            project_id: project.id.clone(),
            display_name: project.display_name,
            active_concurrency,
            running_request_count,
        });
    }
    projects.sort_by(|left, right| {
        right
            .active_concurrency
            .cmp(&left.active_concurrency)
            .then_with(|| right.running_request_count.cmp(&left.running_request_count))
    });

    let breaker_map = read_breaker_open_map(
        redis_pool,
        &provider_rows
            .iter()
            .map(|provider| provider.id.clone())
            .collect::<Vec<_>>(),
    )
    .await;
    let mut providers = Vec::with_capacity(provider_rows.len());
    for provider in provider_rows {
        let running_request_count = provider_counts.get(&provider.id).copied().unwrap_or(0);
        let snapshot = concurrency_snapshots.get(&provider.id);
        let active_concurrency = match snapshot {
            Some(snapshot) => snapshot.active_count,
            // Redis is only consulted when the runtime has no snapshot for this
            // provider. Reporting every provider means the alternative is one
            // round trip per provider on every poll.
            None => read_redis_int(redis_pool, &legacy_provider_concurrency_key(&provider.id))
                .await
                .unwrap_or(running_request_count),
        };
        providers.push(GatewayProviderPressureView {
            provider_account_id: provider.id.clone(),
            label: provider.label.clone(),
            status: provider.status.clone(),
            protocol_family: provider.protocol_family.clone(),
            active_concurrency,
            concurrency_limit: snapshot.map(|snapshot| snapshot.current_limit),
            concurrency_available: snapshot.map(|snapshot| snapshot.available),
            running_request_count,
            breaker_open: breaker_map.get(&provider.id).copied().unwrap_or(false),
        });
    }
    providers.sort_by(|left, right| {
        right
            .active_concurrency
            .cmp(&left.active_concurrency)
            .then_with(|| right.running_request_count.cmp(&left.running_request_count))
            // Idle providers all tie on the counts above, so order them by name to
            // keep the list from reshuffling between polls.
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.provider_account_id.cmp(&right.provider_account_id))
    });

    Ok(GatewayRuntimePressureView {
        total_running_requests: running_rows.len(),
        total_project_concurrency: projects.iter().map(|row| row.active_concurrency).sum(),
        total_provider_concurrency: providers.iter().map(|row| row.active_concurrency).sum(),
        projects,
        providers,
    })
}

async fn read_redis_int(redis_pool: &Pool, key: &str) -> Option<usize> {
    let mut conn = redis_pool.get().await.ok()?;
    let raw = conn.get::<_, Option<String>>(key).await.ok().flatten()?;
    raw.trim()
        .parse::<i64>()
        .ok()
        .map(|value| value.max(0) as usize)
}

fn legacy_provider_concurrency_key(provider_account_id: &str) -> String {
    format!("ai-gateway:provider:{provider_account_id}:concurrency")
}

fn legacy_project_concurrency_key(project_id: &str) -> String {
    format!("ai-gateway:project:{project_id}:concurrency")
}
