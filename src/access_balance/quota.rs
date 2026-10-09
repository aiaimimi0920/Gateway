//! Operator total limits preserve credit already consumed or reserved.
use crate::{db::GatewayAccessKeyBalanceView, error::GatewayError};
use serde::Deserialize;

#[cfg(test)]
#[path = "quota_tests.rs"]
mod tests;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyQuotaInput {
    pub mode: String,
    pub limit: Option<i64>,
    pub currency: Option<String>,
}

impl KeyQuotaInput {
    pub(crate) fn validate_server_mode(
        &self,
        current: Option<&GatewayAccessKeyBalanceView>,
    ) -> Result<(), GatewayError> {
        let mode = current.map(|balance| balance.balance_mode.as_str());
        let mode = if mode == Some("request_prepaid") {
            Some("message_prepaid")
        } else {
            mode
        };
        // PostgreSQL has no durable in-flight counter. Do not let a late finalizer debit
        // another unit after a mode switch; edits to the same total remain safe.
        if matches!(mode, Some("message_prepaid" | "token_prepaid"))
            && mode != Some(self.mode.as_str())
        {
            return Err(GatewayError::conflict(
                "服务器暂不支持切换已有计费密钥的额度类型；可修改总额度，或创建新密钥。",
            )
            .with_code("key_quota_mode_change_unsupported"));
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), GatewayError> {
        let valid = match self.mode.as_str() {
            "unlimited" => self.limit.is_none() && self.currency.is_none(),
            "message_prepaid" | "token_prepaid" => {
                self.limit
                    .is_some_and(|value| (0..=9_007_199_254_740_991).contains(&value))
                    && self.currency.is_none()
            }
            "cash_prepaid" => {
                self.currency.as_deref() == Some("USD")
                    && self
                        .limit
                        .is_some_and(|value| (0..=crate::cash_billing::MAX_MICROS).contains(&value))
            }
            _ => false,
        };
        if !valid {
            return Err(
                GatewayError::bad_request("Invalid key quota mode or total limit")
                    .with_code("invalid_key_quota"),
            );
        }
        Ok(())
    }

    pub(crate) fn apply(
        &self,
        id: &str,
        current: Option<GatewayAccessKeyBalanceView>,
    ) -> Result<GatewayAccessKeyBalanceView, GatewayError> {
        self.validate()?;
        let mut balance = current.unwrap_or_else(|| super::policy::unlimited(id));
        if self.mode != "unlimited" && self.mode != "cash_prepaid" {
            let (total, remaining) = if self.mode == "token_prepaid" {
                (&mut balance.total_tokens, &mut balance.remaining_tokens)
            } else {
                (&mut balance.total_messages, &mut balance.remaining_messages)
            };
            let consumed = total
                .unwrap_or_default()
                .checked_sub(remaining.unwrap_or_default())
                .ok_or_else(|| GatewayError::bad_request("Quota arithmetic overflow"))?
                .max(0);
            let limit = self.limit.expect("validated quota limit");
            *total = Some(limit);
            *remaining = Some(
                limit
                    .checked_sub(consumed)
                    .ok_or_else(|| GatewayError::bad_request("Quota arithmetic overflow"))?,
            );
        }
        balance.balance_mode = self.mode.clone();
        balance.updated_at = super::policy::timestamp();
        Ok(balance)
    }
}
