//! 用户现金应付额的唯一结算 owner；费用估算和供应商余额不参与记账。
pub mod admission;
mod amount;
pub mod finalization;
mod ledger;
mod models;
pub(crate) mod pricing;
pub(crate) mod sql;
pub(crate) mod store;
mod validation;

pub(crate) use amount::{amount_error, multiplier_ppm, MAX_MICROS};
pub(crate) use models::CashOutcome;
pub use models::{CashBalance, CashQuote, CashReceipt};

pub(crate) const SCHEMA: &str = include_str!("../deploy/postgres/initdb/004-gateway-key-cash.sql");

#[cfg(test)]
mod tests;
