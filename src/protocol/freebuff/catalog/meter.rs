//! Narrow typed display meter. Prices are projected once, not locked or represented as actual debits.
use super::BusinessError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset, Weekday};
#[derive(Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum MeterSnapshot {
    Absent,
    Unavailable,
    Available(AvailableMeter),
}
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableMeter {
    pub(super) balance: f64,
    pub(super) daily_remaining: f64,
    pub(super) wallet_balance: f64,
    pub(super) session_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) claimable_grant: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) solo_discount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) quota_exempt: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) spend_without_asking: Option<bool>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireMeter {
    balance: f64,
    daily: Daily,
    wallet: Wallet,
    prices: HashMap<String, f64>,
    claimable_grant_freebucks: Option<f64>,
    solo_discount: Option<f64>,
    quota_exempt: Option<bool>,
    spend_without_asking: Option<bool>,
    #[serde(default)]
    price_changes: Vec<Change>,
    #[serde(default)]
    off_peak: HashMap<String, Offer>,
}
#[derive(Deserialize)]
struct Daily {
    remaining: f64,
}
#[derive(Deserialize)]
struct Wallet {
    balance: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Change {
    at: String,
    model_id: String,
    price: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Offer {
    start_hour_utc: u8,
    end_hour_utc: u8,
    weekends_off_peak: Option<bool>,
    price: f64,
    regular_price: f64,
}
pub(super) fn instant(value: &str) -> Result<OffsetDateTime, BusinessError> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|_| BusinessError::InvalidResponse)
}
fn amount(value: f64) -> Result<(), BusinessError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(BusinessError::InvalidResponse)
    }
}
impl MeterSnapshot {
    pub(super) fn parse(
        body: &serde_json::Value,
        key: &str,
        now: OffsetDateTime,
    ) -> Result<Self, BusinessError> {
        let Some(value) = body.get("freebucks") else {
            return Ok(Self::Absent);
        };
        if value.is_null() {
            return Ok(Self::Unavailable);
        }
        let wire: WireMeter =
            serde_json::from_value(value.clone()).map_err(|_| BusinessError::InvalidResponse)?;
        for value in [wire.balance, wire.daily.remaining, wire.wallet.balance]
            .into_iter()
            .chain(wire.prices.values().copied())
            .chain(wire.claimable_grant_freebucks)
            .chain(wire.solo_discount)
        {
            amount(value)?;
        }
        let mut price = wire.prices.get(key).copied();
        let mut latest = None;
        for change in wire.price_changes {
            amount(change.price)?;
            let at = instant(&change.at)?;
            // Same-time transitions preserve wire order, as the public client's stable sort does.
            if price.is_some()
                && change.model_id == key
                && at <= now
                && latest.is_none_or(|previous| at >= previous)
            {
                price = Some(change.price);
                latest = Some(at);
            }
        }
        for (model, offer) in wire.off_peak {
            amount(offer.price)?;
            amount(offer.regular_price)?;
            if offer.start_hour_utc > 23 || offer.end_hour_utc > 24 {
                return Err(BusinessError::InvalidResponse);
            }
            if model != key || price.is_none() {
                continue;
            }
            let hour = now.to_offset(UtcOffset::UTC).hour();
            let daily = if offer.start_hour_utc < offer.end_hour_utc {
                hour >= offer.start_hour_utc && hour < offer.end_hour_utc
            } else {
                hour >= offer.start_hour_utc || hour < offer.end_hour_utc
            };
            let beijing = now.to_offset(UtcOffset::from_hms(8, 0, 0).expect("fixed valid offset"));
            let weekend = offer.weekends_off_peak == Some(true)
                && matches!(beijing.weekday(), Weekday::Saturday | Weekday::Sunday);
            price = Some(if daily || weekend {
                offer.price
            } else {
                offer.regular_price
            });
        }
        Ok(Self::Available(AvailableMeter {
            balance: wire.balance,
            daily_remaining: wire.daily.remaining,
            wallet_balance: wire.wallet.balance,
            session_price: price,
            claimable_grant: wire.claimable_grant_freebucks,
            solo_discount: wire.solo_discount,
            quota_exempt: wire.quota_exempt,
            spend_without_asking: wire.spend_without_asking,
        }))
    }
}
