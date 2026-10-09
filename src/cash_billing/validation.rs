//! Reject corrupt persisted balances/receipts rather than turning malformed data into credit.
use super::{CashBalance, CashReceipt, MAX_MICROS};
use crate::error::GatewayError;

fn invalid() -> GatewayError {
    GatewayError::server_error("Invalid cash ledger record").with_code("cash_ledger_invalid")
}

fn money(value: i64) -> bool {
    (0..=MAX_MICROS).contains(&value)
}

fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
}

impl CashBalance {
    pub(super) fn validate(&self) -> Result<(), GatewayError> {
        if self.currency != "USD"
            || !money(self.total_micros)
            || !money(self.spent_micros)
            || !money(self.reserved_micros)
            || self.pending_requests > 1024
            || (self.pending_requests == 0 && self.reserved_micros != 0)
        {
            return Err(invalid());
        }
        Ok(())
    }
}

impl CashReceipt {
    pub(super) fn validate(&self) -> Result<(), GatewayError> {
        if self.currency != "USD"
            || !identifier(&self.request_id)
            || !identifier(&self.access_key_id)
            || !money(self.reserved_micros)
            || self.amount_micros.is_some_and(|amount| !money(amount))
            || self.quotes.is_empty()
            || self.quotes.len() > 64
            || time::OffsetDateTime::parse(
                &self.created_at,
                &time::format_description::well_known::Rfc3339,
            )
            .is_err()
        {
            return Err(invalid());
        }
        for quote in &self.quotes {
            if !identifier(&quote.provider_account_id)
                || !identifier(&quote.credential_id)
                || !identifier(&quote.model)
                || !money(quote.prompt_micros_per_1k_tokens)
                || !money(quote.completion_micros_per_1k_tokens)
                || quote.group_multiplier_ppm > 1_000_000_000_000
                || quote.account_multiplier_ppm > 1_000_000_000_000
            {
                return Err(invalid());
            }
        }
        match self.status.as_str() {
            "reserved" | "unresolved" if self.amount_micros.is_none() => {}
            "released" if self.amount_micros == Some(0) => {}
            "settled" => {
                let quote = self.settled_quote.as_ref().ok_or_else(invalid)?;
                let usage = self.usage.as_ref().ok_or_else(invalid)?;
                if !self.quotes.contains(quote)
                    || self.amount_micros != Some(quote.actual_charge(usage)?)
                {
                    return Err(invalid());
                }
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }
}
