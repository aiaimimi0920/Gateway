//! Last observed dollar components -> SDK-reported credits. Never Freebucks or an actual invoice.
use super::{invalid, BusinessError};
use serde_json::Value;
pub(crate) const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Default)]
pub(crate) struct Accounting {
    pub(crate) cost: Option<f64>,
    pub(crate) upstream_estimate: Option<f64>,
}
fn number(value: Option<&Value>) -> Result<Option<f64>, BusinessError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0)
            .map(Some)
            .ok_or_else(invalid),
    }
}
impl Accounting {
    pub(crate) fn observe(&mut self, usage: &Value) -> Result<(), BusinessError> {
        if usage.is_null() {
            return Ok(());
        }
        let usage = usage.as_object().ok_or_else(invalid)?;
        let cost = number(usage.get("cost"))?;
        let estimate = match usage.get("cost_details") {
            None | Some(Value::Null) => None,
            Some(value) => number(
                value
                    .as_object()
                    .ok_or_else(invalid)?
                    .get("upstream_inference_cost"),
            )?,
        };
        // Validate the entire incoming snapshot before replacing either last observed component.
        if let Some(cost) = cost {
            self.cost = Some(cost);
        }
        if let Some(estimate) = estimate {
            self.upstream_estimate = Some(estimate);
        }
        Ok(())
    }
    pub(crate) fn sdk_credits(&self) -> Result<u64, BusinessError> {
        if self.cost.is_none() && self.upstream_estimate.is_none() {
            return Err(BusinessError::AccountingUnknown);
        }
        // Fixed SDK calculateUsedCredits / PROFIT_MARGIN=0.055; sum components once, never chunks.
        let credits = ((self.cost.unwrap_or(0.0) + self.upstream_estimate.unwrap_or(0.0))
            * (1.0 + 0.055)
            * 100.0)
            .round();
        if !credits.is_finite() || !(0.0..=MAX_SAFE_INTEGER as f64).contains(&credits) {
            return Err(BusinessError::AccountingUnknown);
        }
        Ok(credits as u64)
    }
}
impl std::fmt::Debug for Accounting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffAccounting { facts: <redacted> }")
    }
}
