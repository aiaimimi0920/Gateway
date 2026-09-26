use super::*;

pub fn evaluate_credential_count(
    usable_credential_count: usize,
    watermark: CountWatermark,
) -> CountStockEvaluation {
    let deficit_to_min = watermark
        .min
        .map(|minimum| minimum.saturating_sub(usable_credential_count))
        .unwrap_or(0);
    let deficit_to_target = watermark
        .target
        .map(|target| target.saturating_sub(usable_credential_count))
        .unwrap_or(deficit_to_min);
    let excess_over_max = watermark
        .max
        .map(|maximum| usable_credential_count.saturating_sub(maximum))
        .unwrap_or(0);
    let needs_replenishment = deficit_to_min > 0 || deficit_to_target > 0;
    let severity = if deficit_to_min > 0 {
        StockSeverity::Critical
    } else if deficit_to_target > 0 {
        StockSeverity::Warning
    } else if excess_over_max > 0 {
        StockSeverity::Overstock
    } else {
        StockSeverity::Healthy
    };

    CountStockEvaluation {
        usable_credential_count,
        min_credential_count: watermark.min,
        target_credential_count: watermark.target,
        max_credential_count: watermark.max,
        needs_replenishment,
        severity,
        deficit_to_min,
        deficit_to_target,
        suggested_credential_top_up_count: deficit_to_target.max(deficit_to_min),
        excess_over_max,
    }
}

pub fn evaluate_token_window(
    available_tokens_by_credential: &[Option<i64>],
    watermark: TokenWindowWatermark,
) -> TokenWindowStockEvaluation {
    let mut known_sum = 0i64;
    let mut known_count = 0usize;
    let mut unknown_count = 0usize;
    for value in available_tokens_by_credential {
        match value {
            Some(tokens) => {
                known_sum = known_sum.saturating_add((*tokens).max(0));
                known_count += 1;
            }
            None => unknown_count += 1,
        }
    }
    let average_available_tokens = (known_count > 0).then(|| known_sum / known_count as i64);
    let deficit_to_min_average_tokens = average_available_tokens
        .zip(watermark.min_average_available_tokens)
        .map(|(average, minimum)| minimum.saturating_sub(average).max(0))
        .unwrap_or(0);
    let deficit_to_target_average_tokens = average_available_tokens
        .zip(watermark.target_average_available_tokens)
        .map(|(average, target)| target.saturating_sub(average).max(0))
        .unwrap_or(deficit_to_min_average_tokens);
    let credential_count_deficit = watermark
        .target_credential_count
        .map(|target| target.saturating_sub(available_tokens_by_credential.len()))
        .unwrap_or(0);
    let needs_replenishment = deficit_to_min_average_tokens > 0
        || deficit_to_target_average_tokens > 0
        || credential_count_deficit > 0;
    let severity = if known_count == 0
        && (watermark.min_average_available_tokens.is_some()
            || watermark.target_average_available_tokens.is_some())
    {
        StockSeverity::Warning
    } else if deficit_to_min_average_tokens > 0 {
        StockSeverity::Critical
    } else if deficit_to_target_average_tokens > 0 || credential_count_deficit > 0 {
        StockSeverity::Warning
    } else {
        StockSeverity::Healthy
    };

    TokenWindowStockEvaluation {
        known_token_credential_count: known_count,
        unknown_token_credential_count: unknown_count,
        average_available_tokens,
        min_average_available_tokens: watermark.min_average_available_tokens,
        target_average_available_tokens: watermark.target_average_available_tokens,
        target_credential_count: watermark.target_credential_count,
        needs_replenishment,
        severity,
        deficit_to_min_average_tokens,
        deficit_to_target_average_tokens,
        suggested_credential_top_up_count: credential_count_deficit.max(
            if deficit_to_target_average_tokens > 0 {
                1usize.saturating_add(unknown_count)
            } else {
                0
            },
        ),
    }
}

pub(super) fn combined_needs_replenishment(
    count_evaluation: &CountStockEvaluation,
    token_evaluation: Option<&TokenWindowStockEvaluation>,
) -> bool {
    count_evaluation.needs_replenishment
        || token_evaluation
            .map(|evaluation| evaluation.needs_replenishment)
            .unwrap_or(false)
}

pub(super) fn combined_stock_severity(
    count_evaluation: &CountStockEvaluation,
    token_evaluation: Option<&TokenWindowStockEvaluation>,
) -> StockSeverity {
    token_evaluation
        .map(|evaluation| max_severity(count_evaluation.severity, evaluation.severity))
        .unwrap_or(count_evaluation.severity)
}

fn max_severity(left: StockSeverity, right: StockSeverity) -> StockSeverity {
    if severity_rank(right) > severity_rank(left) {
        right
    } else {
        left
    }
}

fn severity_rank(value: StockSeverity) -> u8 {
    match value {
        StockSeverity::Healthy => 0,
        StockSeverity::Overstock => 1,
        StockSeverity::Warning => 2,
        StockSeverity::Critical => 3,
    }
}
