//! Cost overview assembly for database and runtime provider identities.

use super::identity::filter_visible_provider_accounts;
use super::pricing_editor::build_provider_pricing_editor_rows;
use super::usage_queries::{load_provider_model_usage_aggregates, load_provider_supported_models};
use super::*;

pub async fn get_cost_overview(
    pool: &PgPool,
    runtime_providers: &[GatewayRuntimeProviderIdentity],
) -> Result<GatewayCostOverviewView, GatewayError> {
    let (provider_accounts, provider_model_rows, provider_supported_models) = tokio::try_join!(
        list_provider_accounts(pool),
        load_provider_model_usage_aggregates(pool),
        async {
            let provider_accounts = list_provider_accounts(pool).await?;
            let provider_accounts = filter_visible_provider_accounts(provider_accounts);
            let provider_account_ids = provider_accounts
                .iter()
                .map(|provider| provider.id.clone())
                .collect::<Vec<_>>();
            load_provider_supported_models(pool, &provider_account_ids).await
        },
    )?;
    let provider_accounts = filter_visible_provider_accounts(provider_accounts);

    // A database row wins over the route document for the same id: it is the one
    // an operator edits, and it is the only one that can carry pricing.
    let mut cost_providers = provider_accounts
        .iter()
        .map(CostProviderRef::from_account)
        .collect::<Vec<_>>();
    let database_provider_ids = provider_accounts
        .iter()
        .map(|provider| provider.id.as_str())
        .collect::<HashSet<_>>();
    let mut runtime_supported_models = HashMap::<String, Vec<String>>::new();
    for provider in runtime_providers {
        if database_provider_ids.contains(provider.id.as_str()) {
            continue;
        }
        runtime_supported_models.insert(provider.id.clone(), provider.supported_models.clone());
        cost_providers.push(CostProviderRef::from_runtime(provider));
    }

    let mut provider_model_aggregates =
        HashMap::<String, Vec<(String, GatewayUsageAggregate)>>::new();

    for row in provider_model_rows {
        let aggregate = GatewayUsageAggregate {
            request_count: row.request_count.max(0) as usize,
            failure_count: 0,
            recent_request_count_10m: 0,
            recent_failure_count_10m: 0,
            input_tokens: row.input_tokens.max(0),
            output_tokens: row.output_tokens.max(0),
            thinking_tokens: row.thinking_tokens.max(0),
            cached_tokens: row.cached_tokens.max(0),
            prompt_tokens: row.prompt_tokens.max(0),
            completion_tokens: row.completion_tokens.max(0),
            total_tokens: row.total_tokens.max(0),
            last_request_at: row.last_request_at.map(format_timestamp),
            ..GatewayUsageAggregate::default()
        };
        provider_model_aggregates
            .entry(row.provider_account_id)
            .or_default()
            .push((row.model_name, aggregate));
    }

    let mut supported_models = runtime_supported_models;
    supported_models.extend(provider_supported_models);
    build_cost_overview(
        &cost_providers,
        &provider_model_aggregates,
        &supported_models,
    )
}

pub(super) fn build_cost_overview(
    cost_providers: &[CostProviderRef<'_>],
    provider_model_aggregates: &HashMap<String, Vec<(String, GatewayUsageAggregate)>>,
    provider_supported_models: &HashMap<String, Vec<String>>,
) -> Result<GatewayCostOverviewView, GatewayError> {
    let mut provider_buckets = Vec::new();
    let mut model_grouped = HashMap::<String, Vec<GatewayCostModelProviderRowView>>::new();
    let mut pricing_editors = Vec::new();
    let mut total_requests = 0usize;
    let mut total_input_tokens = 0i64;
    let mut total_output_tokens = 0i64;
    let mut total_thinking_tokens = 0i64;
    let mut total_cached_tokens = 0i64;
    let mut total_prompt_tokens = 0i64;
    let mut total_completion_tokens = 0i64;
    let mut total_tokens = 0i64;
    let mut total_estimated_market_cost_micros = 0i64;
    let mut has_any_priced_cost = false;
    let mut priced_provider_count = 0usize;
    let mut model_count = 0usize;
    let mut priced_model_count = 0usize;

    for provider in cost_providers {
        let supported_models = provider_supported_models
            .get(provider.id)
            .cloned()
            .unwrap_or_default();
        let usage_by_model = provider_model_aggregates
            .get(provider.id)
            .cloned()
            .unwrap_or_default();

        let mut usage_lookup = HashMap::<String, GatewayUsageAggregate>::new();
        for (model, aggregate) in usage_by_model {
            usage_lookup.insert(model, aggregate);
        }

        let editor_rows =
            build_provider_pricing_editor_rows(provider, &supported_models, &usage_lookup);
        let configured_model_count = editor_rows
            .iter()
            .filter(|row| row.market_rate.configured)
            .count();
        let model_rows_with_traffic = editor_rows
            .iter()
            .filter(|row| row.request_count > 0 || row.total_tokens > 0)
            .map(|row| GatewayCostProviderModelRowView {
                model: row.model.clone(),
                request_count: row.request_count,
                input_tokens: row.input_tokens,
                output_tokens: row.output_tokens,
                thinking_tokens: row.thinking_tokens,
                cached_tokens: row.cached_tokens,
                prompt_tokens: row.prompt_tokens,
                completion_tokens: row.completion_tokens,
                total_tokens: row.total_tokens,
                market_rate: row.market_rate.clone(),
                estimated_market_cost_micros: row.estimated_market_cost_micros,
                last_request_at: usage_lookup
                    .get(&row.model)
                    .and_then(|aggregate| aggregate.last_request_at.clone()),
            })
            .collect::<Vec<_>>();

        let provider_request_count = model_rows_with_traffic
            .iter()
            .map(|row| row.request_count)
            .sum::<usize>();
        let provider_input_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.input_tokens)
            .sum::<i64>();
        let provider_output_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.output_tokens)
            .sum::<i64>();
        let provider_thinking_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.thinking_tokens)
            .sum::<i64>();
        let provider_cached_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.cached_tokens)
            .sum::<i64>();
        let provider_prompt_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.prompt_tokens)
            .sum::<i64>();
        let provider_completion_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.completion_tokens)
            .sum::<i64>();
        let provider_total_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.total_tokens)
            .sum::<i64>();
        let provider_estimated_market_cost_micros = model_rows_with_traffic
            .iter()
            .map(|row| row.estimated_market_cost_micros.unwrap_or(0))
            .sum::<i64>();
        let provider_has_priced_cost = model_rows_with_traffic
            .iter()
            .any(|row| row.estimated_market_cost_micros.is_some());
        let provider_last_request_at = model_rows_with_traffic
            .iter()
            .filter_map(|row| row.last_request_at.clone())
            .max();

        total_requests += provider_request_count;
        total_input_tokens += provider_input_tokens;
        total_output_tokens += provider_output_tokens;
        total_thinking_tokens += provider_thinking_tokens;
        total_cached_tokens += provider_cached_tokens;
        total_prompt_tokens += provider_prompt_tokens;
        total_completion_tokens += provider_completion_tokens;
        total_tokens += provider_total_tokens;
        if provider_has_priced_cost {
            total_estimated_market_cost_micros += provider_estimated_market_cost_micros;
            has_any_priced_cost = true;
            priced_provider_count += 1;
        }

        for row in &model_rows_with_traffic {
            model_grouped.entry(row.model.clone()).or_default().push(
                GatewayCostModelProviderRowView {
                    provider_account_id: provider.id.to_string(),
                    label: provider.label.to_string(),
                    request_count: row.request_count,
                    input_tokens: row.input_tokens,
                    output_tokens: row.output_tokens,
                    thinking_tokens: row.thinking_tokens,
                    cached_tokens: row.cached_tokens,
                    prompt_tokens: row.prompt_tokens,
                    completion_tokens: row.completion_tokens,
                    total_tokens: row.total_tokens,
                    market_rate: row.market_rate.clone(),
                    estimated_market_cost_micros: row.estimated_market_cost_micros,
                    last_request_at: row.last_request_at.clone(),
                },
            );
        }

        model_count += editor_rows.len();
        priced_model_count += configured_model_count;

        if provider.pricing_editable {
            pricing_editors.push(GatewayProviderPricingEditorView {
                provider_account_id: provider.id.to_string(),
                label: provider.label.to_string(),
                adapter: provider.adapter.to_string(),
                protocol_family: provider.protocol_family.to_string(),
                model_count: editor_rows.len(),
                configured_model_count,
                rows: editor_rows.clone(),
            });
        }

        provider_buckets.push(GatewayCostProviderBucketView {
            provider_account_id: provider.id.to_string(),
            label: provider.label.to_string(),
            adapter: provider.adapter.to_string(),
            protocol_family: provider.protocol_family.to_string(),
            request_count: provider_request_count,
            input_tokens: provider_input_tokens,
            output_tokens: provider_output_tokens,
            thinking_tokens: provider_thinking_tokens,
            cached_tokens: provider_cached_tokens,
            prompt_tokens: provider_prompt_tokens,
            completion_tokens: provider_completion_tokens,
            total_tokens: provider_total_tokens,
            estimated_market_cost_micros: provider_has_priced_cost
                .then_some(provider_estimated_market_cost_micros),
            priced_model_count: configured_model_count,
            unpriced_model_count: editor_rows.len().saturating_sub(configured_model_count),
            last_request_at: provider_last_request_at,
            models: model_rows_with_traffic,
        });
    }

    provider_buckets.sort_by(|left, right| {
        right
            .total_tokens
            .cmp(&left.total_tokens)
            .then_with(|| left.label.cmp(&right.label))
    });

    let mut model_buckets = model_grouped
        .into_iter()
        .map(|(model, mut providers)| {
            providers.sort_by(|left, right| {
                right
                    .total_tokens
                    .cmp(&left.total_tokens)
                    .then_with(|| left.label.cmp(&right.label))
            });
            let request_count = providers.iter().map(|row| row.request_count).sum::<usize>();
            let input_tokens = providers.iter().map(|row| row.input_tokens).sum::<i64>();
            let output_tokens = providers.iter().map(|row| row.output_tokens).sum::<i64>();
            let thinking_tokens = providers.iter().map(|row| row.thinking_tokens).sum::<i64>();
            let cached_tokens = providers.iter().map(|row| row.cached_tokens).sum::<i64>();
            let prompt_tokens = providers.iter().map(|row| row.prompt_tokens).sum::<i64>();
            let completion_tokens = providers
                .iter()
                .map(|row| row.completion_tokens)
                .sum::<i64>();
            let total_tokens = providers.iter().map(|row| row.total_tokens).sum::<i64>();
            let estimated_market_cost_micros = providers
                .iter()
                .map(|row| row.estimated_market_cost_micros.unwrap_or(0))
                .sum::<i64>();
            let priced_provider_count = providers
                .iter()
                .filter(|row| row.market_rate.configured)
                .count();
            let has_priced_cost = providers
                .iter()
                .any(|row| row.estimated_market_cost_micros.is_some());
            let last_request_at = providers
                .iter()
                .filter_map(|row| row.last_request_at.clone())
                .max();
            GatewayCostModelBucketView {
                model,
                request_count,
                input_tokens,
                output_tokens,
                thinking_tokens,
                cached_tokens,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                estimated_market_cost_micros: has_priced_cost
                    .then_some(estimated_market_cost_micros),
                provider_count: providers.len(),
                priced_provider_count,
                last_request_at,
                providers,
            }
        })
        .collect::<Vec<_>>();
    model_buckets.sort_by(|left, right| {
        right
            .total_tokens
            .cmp(&left.total_tokens)
            .then_with(|| left.model.cmp(&right.model))
    });

    pricing_editors.sort_by(|left, right| left.label.cmp(&right.label));

    let provider_count = cost_providers.len();
    let unpriced_provider_count = provider_count.saturating_sub(priced_provider_count);
    let unpriced_model_count = model_count.saturating_sub(priced_model_count);

    Ok(GatewayCostOverviewView {
        summary: GatewayCostOverviewSummaryView {
            provider_count,
            priced_provider_count,
            unpriced_provider_count,
            model_count,
            priced_model_count,
            unpriced_model_count,
            total_requests,
            total_input_tokens,
            total_output_tokens,
            total_thinking_tokens,
            total_cached_tokens,
            total_prompt_tokens,
            total_completion_tokens,
            total_tokens,
            estimated_market_cost_micros: has_any_priced_cost
                .then_some(total_estimated_market_cost_micros),
        },
        provider_buckets,
        model_buckets,
        pricing_editors,
    })
}
