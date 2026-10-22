//! Apply advisory dimensions only after eligibility filtering; keep projected rows aligned.
use super::*;
use crate::routing::smart::{tier, Dimension, Order};
use sha2::{Digest, Sha256};

pub(crate) fn session_scope(ctx: &PipelineContext) -> Option<String> {
    let session = ctx.canonical_req.explicit_session_key.as_deref()?.trim();
    if session.is_empty() {
        return None;
    }
    let identity = ctx.session.as_ref()?;
    let key = serde_json::json!([
        identity.tenant_id,
        identity.project_id,
        identity.access_key_id,
        identity.api_key_id,
        identity.user_credential_id,
        session
    ]);
    Some(format!("{:x}", Sha256::digest(key.to_string().as_bytes())))
}

pub(super) async fn apply(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
) -> Result<(), GatewayError> {
    let runtime = &state.lifecycle.smart_routing;
    let policy = runtime.policy().map_err(GatewayError::bad_request)?;
    let key_order = if let (Some(local), Some(id)) = (
        &state.local_runtime,
        crate::local_runtime::access_keys::local_key_id(state, ctx.session.as_ref()),
    ) {
        local.routing_order(id).await?
    } else if let (Some(pool), Some(session)) = (&state.pg_pool, &ctx.session) {
        if let Some(id) = session.access_key_id.as_deref() {
            let value: Option<Option<serde_json::Value>> = sqlx::query_scalar(
                "SELECT metadata->'routingOrder' FROM gateway_access_keys WHERE id=$1 AND resolved_tenant_id=$2 AND resolved_project_id=$3 AND status='active'")
                .bind(id).bind(&session.tenant_id).bind(&session.project_id).fetch_optional(pool).await
                .map_err(|_| GatewayError::server_error("Cannot read access key routing policy"))?;
            value
                .flatten()
                .filter(|v| !v.is_null())
                .map(serde_json::from_value::<Order>)
                .transpose()
                .map_err(|_| GatewayError::bad_request("Invalid access key routingOrder"))?
        } else {
            None
        }
    } else {
        None
    };
    let order = if let Some(header) = ctx.request_headers.get("x-gateway-routing-order") {
        serde_json::from_value::<Order>(serde_json::json!(header
            .split(',')
            .map(str::trim)
            .collect::<Vec<_>>()))
        .map_err(|_| GatewayError::bad_request("Invalid routing order"))?
    } else {
        key_order.unwrap_or_else(|| policy.order.clone())
    };
    let model = ctx
        .canonical_req
        .requested_model
        .as_deref()
        .unwrap_or_default();
    let sticky = runtime.sticky(session_scope(ctx).as_deref(), model);
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let snapshot = state.route_config.snapshot();
    let mut scored = Vec::new();
    for (index, candidate) in ctx.candidates.iter().enumerate() {
        let measured = policy
            .measurements
            .iter()
            .find(|row| row.provider == candidate.provider_account_id && row.model == model);
        let intelligence = measured
            .filter(|row| row.valid_until_unix.is_none_or(|expiry| expiry > now))
            .and_then(|row| row.intelligence)
            .unwrap_or(100.0);
        let mut price = f64::INFINITY;
        if let Some(row) = measured {
            if let (Some(input), Some(output)) = (row.input_per_million, row.output_per_million) {
                price = input + output;
            }
        }
        if !price.is_finite() {
            if let Ok(payload) =
                crate::cash_billing::pricing::payload(state, &candidate.provider_account_id).await
            {
                if let Ok(quote) = crate::cash_billing::pricing::quote(
                    &snapshot,
                    candidate,
                    model,
                    None,
                    ctx.account_group_id.as_deref(),
                    &payload,
                ) {
                    // Equal input/output workload; all candidates use the same units and weights.
                    price = (quote.prompt_micros_per_1k_tokens as f64
                        + quote.completion_micros_per_1k_tokens as f64)
                        * quote.group_multiplier_ppm as f64
                        / 1_000_000.0
                        * quote.account_multiplier_ppm as f64
                        / 1_000_000.0
                        / 1000.0;
                }
            }
        }
        let blocked = candidate.routing_score.is_some_and(|score| score <= 0.0)
            || candidate
                .cooldown_until
                .as_deref()
                .and_then(|expiry| {
                    time::OffsetDateTime::parse(
                        expiry,
                        &time::format_description::well_known::Rfc3339,
                    )
                    .ok()
                })
                .is_some_and(|expiry| expiry.unix_timestamp() > now);
        scored.push((
            index,
            blocked,
            sticky.as_deref() == Some(&candidate.provider_account_id),
            tier(runtime.score(&candidate.provider_account_id, model)),
            tier(intelligence),
            price,
        ));
    }
    scored.sort_by(|a, b| {
        let eligible = a.1.cmp(&b.1);
        if !eligible.is_eq() {
            return eligible;
        }
        if !a.1 && a.2 != b.2 {
            return b.2.cmp(&a.2);
        }
        for dimension in &order.0 {
            let result = match dimension {
                Dimension::Availability => b.3.cmp(&a.3),
                Dimension::Intelligence => b.4.cmp(&a.4),
                Dimension::Price => a.5.total_cmp(&b.5),
            };
            if !result.is_eq() {
                return result;
            }
        }
        a.0.cmp(&b.0)
    });
    let candidates = ctx.candidates.clone();
    let projected = ctx.projected_access_candidates.clone();
    ctx.candidates = scored.iter().map(|s| candidates[s.0].clone()).collect();
    if !projected.is_empty() {
        if projected.len() != candidates.len() {
            return Err(GatewayError::server_error(
                "Projected routing rows are misaligned",
            ));
        }
        ctx.projected_access_candidates = scored.iter().map(|s| projected[s.0].clone()).collect();
    }
    ctx.route_selection_strategy = Some(format!(
        "sticky_then_{}",
        order
            .0
            .iter()
            .map(|d| match d {
                Dimension::Availability => "availability",
                Dimension::Intelligence => "intelligence",
                Dimension::Price => "price",
            })
            .collect::<Vec<_>>()
            .join("_")
    ));
    Ok(())
}
