//! Consolidated integration contract suite (runtime_and_storage) to reduce redundant binary linking.

#[path = "consolidated_suites/runtime_and_storage/nous_portal_runtime_contract.rs"]
mod nous_portal_runtime_contract;
#[path = "consolidated_suites/runtime_and_storage/qwen_workspace_runtime_contract.rs"]
mod qwen_workspace_runtime_contract;
#[path = "consolidated_suites/runtime_and_storage/request_client_profiles_runtime_contract.rs"]
mod request_client_profiles_runtime_contract;
#[path = "consolidated_suites/runtime_and_storage/sqlx_runtime_contract.rs"]
mod sqlx_runtime_contract;
