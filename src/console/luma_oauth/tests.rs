//! Real management and loopback flows against synthetic token peers, never a live Luma account.
#[path = "../../../tests/fixtures/luma_mcp.rs"]
mod account;
mod fixture;
mod lifecycle;
mod management;
mod offline_management;
mod offline_storage;
mod refresh_fixture;
mod refresh_management;
mod refresh_recovery;
mod refresh_save_retry;
mod registration;
mod registration_offline;
mod registration_recovery;
mod registration_store;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
