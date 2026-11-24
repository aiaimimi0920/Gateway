//! Consolidated integration contract suite (routing_and_console) to reduce redundant binary linking.

#[path = "consolidated_suites/routing_and_console/bedrock_console_keepalive_contract.rs"]
mod bedrock_console_keepalive_contract;
#[path = "consolidated_suites/routing_and_console/bedrock_console_secret_contract.rs"]
mod bedrock_console_secret_contract;
#[path = "consolidated_suites/routing_and_console/console_test_plan_multi_account_contract.rs"]
mod console_test_plan_multi_account_contract;
#[path = "consolidated_suites/routing_and_console/dim_model_routing_contract.rs"]
mod dim_model_routing_contract;
#[path = "consolidated_suites/routing_and_console/provider_discovery_alias_contract.rs"]
mod provider_discovery_alias_contract;
#[path = "consolidated_suites/routing_and_console/qwen_catalog_binding_contract.rs"]
mod qwen_catalog_binding_contract;
#[path = "consolidated_suites/routing_and_console/retired_provider_preset_contract.rs"]
mod retired_provider_preset_contract;
#[path = "consolidated_suites/routing_and_console/route_credential_enabled_contract.rs"]
mod route_credential_enabled_contract;
