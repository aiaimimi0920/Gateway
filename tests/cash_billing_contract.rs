//! Real management/routing/SQLite contracts; every upstream is a disposable loopback fixture.
#[path = "cash_billing_contract/default_group.rs"]
mod default_group;
#[path = "cash_billing_contract/fixture.rs"]
mod fixture;
#[path = "cash_billing_contract/key_status.rs"]
mod key_status;
#[path = "cash_billing_contract/lifecycle.rs"]
mod lifecycle;
#[path = "cash_billing_contract/pricing_read.rs"]
mod pricing_read;
#[path = "cash_billing_contract/safety.rs"]
mod safety;
#[path = "cash_billing_contract/settlement.rs"]
mod settlement;
#[path = "console_contract_support/mod.rs"]
mod support;
#[path = "cash_billing_contract/upstream.rs"]
mod upstream;
