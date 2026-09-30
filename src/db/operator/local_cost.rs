//! The local source feeds the same pricing and cost projection as PostgreSQL.
use super::*;
use crate::local_runtime::{storage_error, LocalRuntime};

#[derive(sqlx::FromRow)]
struct LocalUsage {
    provider_id: String,
    model: String,
    requests: i64,
    prompt: i64,
    completion: i64,
    total: i64,
    cached: i64,
    last_request: String,
}

pub async fn get_local_cost_overview(
    db: &LocalRuntime,
    providers: &[GatewayRuntimeProviderIdentity],
) -> Result<GatewayCostOverviewView, GatewayError> {
    let rows = sqlx::query_as::<_, LocalUsage>("SELECT
        json_extract(payload, '$.providerAccountId') AS provider_id,
        coalesce(nullif(trim(json_extract(payload, '$.resolvedModel')), ''),
            nullif(trim(json_extract(payload, '$.requestedModel')), ''),
            nullif(trim(json_extract(payload, '$.modelAlias')), ''), 'unknown') AS model,
        count(*) AS requests, coalesce(sum(json_extract(payload, '$.promptTokens')), 0) AS prompt,
        coalesce(sum(json_extract(payload, '$.completionTokens')), 0) AS completion,
        coalesce(sum(coalesce(json_extract(payload, '$.totalTokens'),
            coalesce(json_extract(payload, '$.promptTokens'), 0) + coalesce(json_extract(payload, '$.completionTokens'), 0))), 0) AS total,
        coalesce(sum(coalesce(json_extract(payload, '$.cacheReadInputTokens'), 0) +
            coalesce(json_extract(payload, '$.cacheCreationInputTokens'), 0)), 0) AS cached,
        max(created) AS last_request FROM request_audits
        WHERE json_extract(payload, '$.providerAccountId') IS NOT NULL
        GROUP BY provider_id, model")
        .fetch_all(&db.pool).await.map_err(storage_error)?;
    let pricing_rows: Vec<(String, String)> =
        sqlx::query_as("SELECT provider_id, payload FROM provider_pricing")
            .fetch_all(&db.pool)
            .await
            .map_err(storage_error)?;
    let mut pricing = HashMap::<String, Value>::new();
    for (id, payload) in pricing_rows {
        pricing.insert(
            id,
            serde_json::from_str(&payload)
                .map_err(|_| GatewayError::server_error("Local pricing data is corrupt"))?,
        );
    }
    let mut usage = HashMap::<String, Vec<(String, GatewayUsageAggregate)>>::new();
    for row in rows {
        usage.entry(row.provider_id).or_default().push((
            row.model,
            GatewayUsageAggregate {
                request_count: row.requests.max(0) as usize,
                input_tokens: row.prompt,
                output_tokens: row.completion,
                cached_tokens: row.cached,
                prompt_tokens: row.prompt,
                completion_tokens: row.completion,
                total_tokens: row.total,
                last_request_at: Some(row.last_request),
                ..Default::default()
            },
        ));
    }
    // Keep history visible after a provider is removed from the route document.
    let mut providers = providers.to_vec();
    for id in usage.keys() {
        if !providers.iter().any(|provider| &provider.id == id) {
            providers.push(GatewayRuntimeProviderIdentity {
                id: id.clone(),
                label: id.clone(),
                status: "deleted".into(),
                adapter: String::new(),
                protocol_family: String::new(),
                supported_models: Vec::new(),
            });
        }
    }
    let refs = providers
        .iter()
        .map(|provider| CostProviderRef {
            pricing_payload: pricing.get(&provider.id),
            pricing_editable: provider.status != "deleted",
            ..CostProviderRef::from_runtime(provider)
        })
        .collect::<Vec<_>>();
    let models = providers
        .iter()
        .map(|provider| (provider.id.clone(), provider.supported_models.clone()))
        .collect();
    cost::build_cost_overview(&refs, &usage, &models)
}
