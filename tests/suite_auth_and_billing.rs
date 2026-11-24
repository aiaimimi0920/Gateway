//! Consolidated integration contract suite (auth_and_billing) to reduce redundant binary linking.

#[path = "consolidated_suites/auth_and_billing/access_balance_contract.rs"]
mod access_balance_contract;
#[path = "consolidated_suites/auth_and_billing/kiro_oauth_runtime_contract.rs"]
mod kiro_oauth_runtime_contract;
#[path = "consolidated_suites/auth_and_billing/phase3_access_reliability_contract.rs"]
mod phase3_access_reliability_contract;
#[path = "consolidated_suites/auth_and_billing/request_fallback_authorization_contract.rs"]
mod request_fallback_authorization_contract;
