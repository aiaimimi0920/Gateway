use super::{amount, amount_error};
use crate::{error::GatewayError, protocol::canonical::TokenUsage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CashBalance {
    pub currency: String,
    pub total_micros: i64,
    pub spent_micros: i64,
    pub reserved_micros: i64,
    pub pending_requests: u64,
}

impl CashBalance {
    pub(crate) fn new(total_micros: i64) -> Self {
        Self {
            currency: "USD".into(),
            total_micros,
            spent_micros: 0,
            reserved_micros: 0,
            pending_requests: 0,
        }
    }

    pub fn remaining_micros(&self) -> i64 {
        self.total_micros
            .saturating_sub(self.spent_micros)
            .saturating_sub(self.reserved_micros)
    }

    pub(crate) fn reserve(&mut self, amount: i64) -> Result<(), GatewayError> {
        if amount < 0 || amount > self.remaining_micros() || self.pending_requests >= 1024 {
            return Err(GatewayError::quota_exceeded("当前 Key 的现金额度不足")
                .with_code("cash_balance_exhausted"));
        }
        self.reserved_micros = self
            .reserved_micros
            .checked_add(amount)
            .ok_or_else(amount_error)?;
        self.pending_requests += 1;
        Ok(())
    }

    pub(crate) fn settle(&mut self, reserved: i64, actual: i64) -> Result<(), GatewayError> {
        if reserved > self.reserved_micros
            || reserved < 0
            || actual < 0
            || self.pending_requests == 0
        {
            return Err(GatewayError::server_error("Cash ledger invariant violated"));
        }
        self.spent_micros = self
            .spent_micros
            .checked_add(actual)
            .filter(|value| *value <= super::MAX_MICROS)
            .ok_or_else(amount_error)?;
        self.reserved_micros -= reserved;
        self.pending_requests -= 1;
        Ok(())
    }
}

/// No credentials or request content are stored with this frozen tariff.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CashQuote {
    pub provider_account_id: String,
    pub credential_id: String,
    pub model: String,
    pub group_id: Option<String>,
    pub price_source: String,
    pub prompt_micros_per_1k_tokens: i64,
    pub completion_micros_per_1k_tokens: i64,
    pub group_multiplier_ppm: u64,
    pub account_multiplier_ppm: u64,
    pub cache_tokens_separate: bool,
}

impl CashQuote {
    pub(crate) fn actual_charge(&self, usage: &TokenUsage) -> Result<i64, GatewayError> {
        let prompt = if self.cache_tokens_separate {
            usage
                .prompt_tokens
                .checked_add(usage.cache_read_input_tokens.unwrap_or(0))
                .and_then(|value| value.checked_add(usage.cache_creation_input_tokens.unwrap_or(0)))
                .ok_or_else(amount_error)?
        } else {
            usage.prompt_tokens
        };
        self.charge(prompt, usage.completion_tokens)
    }

    pub(crate) fn charge(&self, prompt: u64, completion: u64) -> Result<i64, GatewayError> {
        amount::charge(
            prompt,
            completion,
            self.prompt_micros_per_1k_tokens,
            self.completion_micros_per_1k_tokens,
            self.group_multiplier_ppm,
            self.account_multiplier_ppm,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashReceipt {
    pub request_id: String,
    pub access_key_id: String,
    pub created_at: String,
    pub status: String,
    pub reserved_micros: i64,
    pub amount_micros: Option<i64>,
    pub currency: String,
    pub quotes: Vec<CashQuote>,
    pub settled_quote: Option<CashQuote>,
    pub usage: Option<TokenUsage>,
    pub reason: Option<String>,
}

pub(crate) enum CashOutcome {
    Success {
        provider: String,
        credential: String,
        model: String,
        usage: Option<TokenUsage>,
    },
    NotSent,
    Unknown,
}
