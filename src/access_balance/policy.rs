//! Shared balance rules. Storage adapters run these inside their own transactions.
use crate::db::{AccessBalanceDecision, AccessKeyBalanceAdjustInput, GatewayAccessKeyBalanceView};
use crate::error::GatewayError;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub(super) fn timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("UTC timestamp")
}

pub(crate) fn unlimited(id: &str) -> GatewayAccessKeyBalanceView {
    GatewayAccessKeyBalanceView {
        access_key_id: id.into(),
        balance_mode: "unlimited".into(),
        status: "active".into(),
        unlimited_until: None,
        period_starts_at: None,
        period_ends_at: None,
        total_tokens: None,
        remaining_tokens: None,
        total_messages: None,
        remaining_messages: None,
        cash: None,
        updated_at: timestamp(),
    }
}

fn parse(value: Option<&str>) -> Result<Option<OffsetDateTime>, GatewayError> {
    value
        .map(|value| {
            OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
                GatewayError::bad_request("Balance timestamps must be RFC3339")
                    .with_code("invalid_balance_timestamp")
            })
        })
        .transpose()
}

pub(crate) fn evaluate(
    balance: &GatewayAccessKeyBalanceView,
    estimate: u64,
) -> AccessBalanceDecision {
    let mut decision = AccessBalanceDecision {
        allowed: false,
        balance_mode: Some(balance.balance_mode.clone()),
        pre_deduct_amount: 0,
        remaining_tokens: balance.remaining_tokens,
        remaining_messages: balance.remaining_messages,
        reason: None,
    };
    let reason = if balance.status != "active" {
        Some("balance_inactive")
    } else {
        let now = OffsetDateTime::now_utc();
        let period = parse(balance.period_starts_at.as_deref())
            .and_then(|start| parse(balance.period_ends_at.as_deref()).map(|end| (start, end)));
        match period {
            Err(_) => Some("balance_period_invalid"),
            Ok((start, end))
                if start.is_some_and(|value| now < value)
                    || end.is_some_and(|value| now >= value) =>
            {
                Some("balance_period_inactive")
            }
            Ok(_) => match balance.balance_mode.as_str() {
                "unlimited" => None,
                // Cash admission is atomic after route/tariff resolution, before dispatch.
                "cash_prepaid" => None,
                "time_pass" => match parse(balance.unlimited_until.as_deref()) {
                    Ok(Some(expiry)) if expiry > now => None,
                    _ => Some("time_pass_inactive"),
                },
                "message_prepaid" | "request_prepaid" => {
                    if balance.remaining_messages.unwrap_or(0) > 0 {
                        decision.balance_mode = Some("message_prepaid".into());
                        decision.pre_deduct_amount = 1;
                        None
                    } else {
                        Some("message_balance_exhausted")
                    }
                }
                "token_prepaid" => match i64::try_from(estimate.max(1)) {
                    Ok(amount) if balance.remaining_tokens.unwrap_or(0) >= amount => {
                        decision.pre_deduct_amount = estimate.max(1);
                        None
                    }
                    _ => Some("token_balance_exhausted"),
                },
                _ => Some("balance_mode_invalid"),
            },
        }
    };
    decision.allowed = reason.is_none();
    decision.reason = reason.map(str::to_owned);
    decision
}

fn add(value: &mut Option<i64>, delta: i64) -> Result<(), GatewayError> {
    if let Some(value) = value {
        *value = value.checked_add(delta).ok_or_else(|| {
            GatewayError::bad_request("Balance arithmetic overflow").with_code("balance_overflow")
        })?;
    }
    Ok(())
}

fn amount(value: u64) -> Result<i64, GatewayError> {
    i64::try_from(value).map_err(|_| {
        GatewayError::bad_request("Balance amount is out of range").with_code("balance_overflow")
    })
}

pub(super) fn reserve(
    balance: &mut GatewayAccessKeyBalanceView,
    estimate: u64,
) -> Result<AccessBalanceDecision, GatewayError> {
    let decision = evaluate(balance, estimate);
    if decision.allowed {
        refund(balance, decision.pre_deduct_amount, false)?;
    }
    Ok(decision)
}

pub(super) fn refund(
    balance: &mut GatewayAccessKeyBalanceView,
    reserved: u64,
    refund: bool,
) -> Result<(), GatewayError> {
    let delta = amount(reserved)? * if refund { 1 } else { -1 };
    match balance.balance_mode.as_str() {
        "token_prepaid" => add(&mut balance.remaining_tokens, delta)?,
        "message_prepaid" | "request_prepaid" => add(&mut balance.remaining_messages, delta)?,
        _ => {}
    }
    balance.updated_at = timestamp();
    Ok(())
}

pub(super) fn settle(
    balance: &mut GatewayAccessKeyBalanceView,
    reserved: u64,
    actual: u64,
) -> Result<(), GatewayError> {
    if balance.balance_mode == "token_prepaid" {
        let delta = amount(reserved)?
            .checked_sub(amount(actual)?)
            .ok_or_else(|| {
                GatewayError::bad_request("Balance arithmetic overflow")
                    .with_code("balance_overflow")
            })?;
        // Refunds and settlements change remaining credit, never purchased totals.
        add(&mut balance.remaining_tokens, delta)?;
        balance.updated_at = timestamp();
    }
    Ok(())
}

pub(super) fn adjust(
    id: &str,
    current: Option<GatewayAccessKeyBalanceView>,
    input: AccessKeyBalanceAdjustInput,
) -> Result<GatewayAccessKeyBalanceView, GatewayError> {
    let mut balance = current.unwrap_or_else(|| {
        let mut value = unlimited(id);
        value.balance_mode = "token_prepaid".into();
        value
    });
    if let Some(mode) = input.balance_mode {
        let mode = mode.trim().to_ascii_lowercase();
        balance.balance_mode = match mode.as_str() {
            "request_prepaid" => "message_prepaid".into(),
            "unlimited" | "time_pass" | "message_prepaid" | "token_prepaid" => mode.trim().into(),
            _ => {
                return Err(GatewayError::bad_request("Invalid balance mode")
                    .with_code("balance_mode_invalid"))
            }
        };
    }
    if let Some(status) = input.status {
        if !matches!(
            status.as_str(),
            "active" | "inactive" | "suspended" | "expired"
        ) {
            return Err(GatewayError::bad_request("Invalid balance status"));
        }
        balance.status = status;
    }
    for (target, value) in [
        (&mut balance.unlimited_until, input.unlimited_until),
        (&mut balance.period_starts_at, input.period_starts_at),
        (&mut balance.period_ends_at, input.period_ends_at),
    ] {
        if value.is_some() {
            parse(value.as_deref())?;
            *target = value;
        }
    }
    let start = parse(balance.period_starts_at.as_deref())?;
    let end = parse(balance.period_ends_at.as_deref())?;
    if start.zip(end).is_some_and(|(start, end)| start >= end) {
        return Err(GatewayError::bad_request(
            "Balance period must end after it starts",
        ));
    }
    if let Some(total) = input.total_tokens {
        if total < 0 {
            return Err(GatewayError::bad_request("Total tokens cannot be negative"));
        }
        balance.total_tokens = Some(total);
        balance.remaining_tokens.get_or_insert(total);
    }
    if let Some(total) = input.total_messages {
        if total < 0 {
            return Err(GatewayError::bad_request(
                "Total messages cannot be negative",
            ));
        }
        balance.total_messages = Some(total);
        balance.remaining_messages.get_or_insert(total);
    }
    if input.remaining_tokens.is_some() {
        balance.remaining_tokens = input.remaining_tokens;
    }
    if input.remaining_messages.is_some() {
        balance.remaining_messages = input.remaining_messages;
    }
    let tokens = input.token_delta.unwrap_or_default();
    let messages = input.message_delta.unwrap_or_default();
    if tokens != 0 {
        balance.remaining_tokens.get_or_insert(0);
        balance.total_tokens.get_or_insert(0);
        add(&mut balance.remaining_tokens, tokens)?;
        add(&mut balance.total_tokens, tokens.max(0))?;
    }
    if messages != 0 {
        balance.remaining_messages.get_or_insert(0);
        balance.total_messages.get_or_insert(0);
        add(&mut balance.remaining_messages, messages)?;
        add(&mut balance.total_messages, messages.max(0))?;
    }
    balance.updated_at = timestamp();
    Ok(balance)
}
