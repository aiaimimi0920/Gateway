//! End is not refund: keep unknown amounts, pending settlement and grace distinct from final receipts.
use super::{BusinessError, BusinessReply, SessionClaim};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RefundSources {
    pub(crate) daily: f64,
    pub(crate) wallet: f64,
}
/// Sanitized facts, not an end/admission capability. Only a freshly executed exact-claim DELETE
/// may authorize local closeout; deserializing an archived receipt never does.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SessionEndReceipt {
    pub(crate) refund: Option<f64>,
    pub(crate) refund_sources: Option<RefundSources>,
    refund_pending: bool,
    in_grace: bool,
}
fn amount(value: &Value) -> Result<f64, BusinessError> {
    value
        .as_f64()
        .filter(|n| n.is_finite() && *n >= 0.0)
        .ok_or(BusinessError::InvalidResponse)
}
impl SessionEndReceipt {
    pub(crate) fn parse(reply: BusinessReply, claim: &SessionClaim) -> Result<Self, BusinessError> {
        let body = reply
            .body
            .as_object()
            .ok_or(BusinessError::InvalidResponse)?;
        if reply.status != 200 || body.get("status").and_then(Value::as_str) != Some("ended") {
            return Err(BusinessError::InvalidResponse);
        }
        let refund = body.get("freebucksRefund").map(amount).transpose()?;
        let refund_pending = body
            .get("freebucksRefundPending")
            .map(|v| v.as_bool().ok_or(BusinessError::InvalidResponse))
            .transpose()?
            .unwrap_or(false);
        let in_grace = match body.get("instanceId") {
            None => false,
            Some(value) if value.as_str() == Some(claim.instance_id()) => true,
            Some(_) => return Err(BusinessError::InvalidResponse),
        };
        let refund_sources = body
            .get("freebucksRefundSources")
            .map(|value| {
                Ok::<_, BusinessError>(RefundSources {
                    daily: amount(value.get("daily").ok_or(BusinessError::InvalidResponse)?)?,
                    wallet: amount(value.get("wallet").ok_or(BusinessError::InvalidResponse)?)?,
                })
            })
            .transpose()?;
        Ok(Self {
            refund,
            refund_sources,
            refund_pending,
            in_grace,
        })
    }
    pub(crate) fn is_final(&self) -> bool {
        self.refund.is_some() && !self.refund_pending && !self.in_grace
    }
    pub(crate) fn restore(bytes: &[u8]) -> Result<Self, BusinessError> {
        let receipt: Self =
            serde_json::from_slice(bytes).map_err(|_| BusinessError::InvalidResponse)?;
        let valid_amount = |n: f64| n.is_finite() && n >= 0.0;
        if !receipt.is_final()
            || !receipt.refund.is_some_and(valid_amount)
            || receipt
                .refund_sources
                .as_ref()
                .is_some_and(|s| !valid_amount(s.daily) || !valid_amount(s.wallet))
        {
            return Err(BusinessError::InvalidResponse);
        }
        Ok(receipt)
    }
}
