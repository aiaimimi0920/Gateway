#![recursion_limit = "256"]

// Unit tests reuse the same synthetic management fixture as integration contracts.
#[cfg(test)]
extern crate self as neuro_gateway;

#[cfg(test)]
#[path = "../tests/gemini_canvas_host_ingress_contract/http_contract.rs"]
mod canvas_http_ingress_contract;

pub mod access_balance;
pub use gateway_core::access_control;
pub mod access_key_groups;
pub mod access_store;
pub mod auth;
pub use gateway_core::balance;
pub mod browser_executor_runtime;
pub mod cash_billing;
pub use gateway_core::concurrency;
pub mod config;
pub mod console;
pub mod conversation_archive;
pub mod conversation_dataset;
pub mod credential_pool_automation;
pub(crate) mod credential_pool_storage;
pub mod credential_refill;
pub use gateway_core::credential_runtime;
pub mod credential_stock;
pub mod credential_store;
pub mod credential_test_plan;
pub mod credential_test_policy;
pub mod db;
pub use gateway_core::error;
pub use gateway_core::filter;
pub use gateway_core::gateway_api_key;
pub mod http;
pub mod http_client;
pub mod implementation_lines;
pub mod keepalive;
pub mod local_runtime;
mod luma_reconciliation;
pub(crate) mod luma_web_session;
pub mod metrics;
pub mod object_storage;
pub mod pipeline;
pub mod preset;
pub mod protocol;
pub mod provider_credential_folder_sync;
pub mod provider_credential_probe_scheduler;
pub mod provider_credential_refresh;
pub mod provider_discovery;
pub use gateway_core::provider_failure;
pub use gateway_core::provider_health_state;
pub(crate) use gateway_core::provider_identity;
pub(crate) mod provider_model_id;
pub mod provider_quota;
pub mod provider_runtime;
pub mod rate_limit;
pub mod redis;
pub mod request_wait;
pub use gateway_core::retry;
pub use gateway_provider_kit as provider_kit;
pub mod routing;
pub mod runtime;
pub mod site_health;
pub mod splitter;
pub mod state;
pub mod token_refresh;
pub mod upstream;
pub mod usage_aggregation;
