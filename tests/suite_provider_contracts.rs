//! Consolidated integration contract suite (provider_contracts) to reduce redundant binary linking.

#[path = "consolidated_suites/provider_contracts/accio_revision_compatibility.rs"]
mod accio_revision_compatibility;
#[path = "consolidated_suites/provider_contracts/attempt_cancellation_metrics_contract.rs"]
mod attempt_cancellation_metrics_contract;
#[path = "consolidated_suites/provider_contracts/basispoints_channel_contract.rs"]
mod basispoints_channel_contract;
#[path = "consolidated_suites/provider_contracts/basispoints_stream_bridge_contract.rs"]
mod basispoints_stream_bridge_contract;
#[path = "consolidated_suites/provider_contracts/basispoints_wire_contract.rs"]
mod basispoints_wire_contract;
#[path = "consolidated_suites/provider_contracts/bedrock_converse_contract.rs"]
mod bedrock_converse_contract;
#[path = "consolidated_suites/provider_contracts/bedrock_credential_mode_contract.rs"]
mod bedrock_credential_mode_contract;
#[path = "consolidated_suites/provider_contracts/bedrock_generation_parameters_contract.rs"]
mod bedrock_generation_parameters_contract;
#[path = "consolidated_suites/provider_contracts/bedrock_material_roundtrip_contract.rs"]
mod bedrock_material_roundtrip_contract;
#[path = "consolidated_suites/provider_contracts/claude_wire_contract.rs"]
mod claude_wire_contract;
#[path = "consolidated_suites/provider_contracts/codex_native_fidelity_contract.rs"]
mod codex_native_fidelity_contract;
#[path = "consolidated_suites/provider_contracts/conversation_archive_contract.rs"]
mod conversation_archive_contract;
#[path = "consolidated_suites/provider_contracts/copilot_request_contract.rs"]
mod copilot_request_contract;
#[path = "consolidated_suites/provider_contracts/cpa_profile_contract.rs"]
mod cpa_profile_contract;
#[path = "consolidated_suites/provider_contracts/cpa_wire_contract.rs"]
mod cpa_wire_contract;
#[path = "consolidated_suites/provider_contracts/default_account_group_contract.rs"]
mod default_account_group_contract;
#[path = "consolidated_suites/provider_contracts/grok_web_typed_contract.rs"]
mod grok_web_typed_contract;
#[path = "consolidated_suites/provider_contracts/html_upstream_rejection.rs"]
mod html_upstream_rejection;
#[path = "consolidated_suites/provider_contracts/http_client_body_contract.rs"]
mod http_client_body_contract;
#[path = "consolidated_suites/provider_contracts/http_client_classification_contract.rs"]
mod http_client_classification_contract;
#[path = "consolidated_suites/provider_contracts/http_client_tls_contract.rs"]
mod http_client_tls_contract;
#[path = "consolidated_suites/provider_contracts/ingress_extractor_limits.rs"]
mod ingress_extractor_limits;
#[path = "consolidated_suites/provider_contracts/kiro_api_key_secret_contract.rs"]
mod kiro_api_key_secret_contract;
#[path = "consolidated_suites/provider_contracts/latency_histogram_contract.rs"]
mod latency_histogram_contract;
#[path = "consolidated_suites/provider_contracts/luma_agents_config_contract.rs"]
mod luma_agents_config_contract;
#[path = "consolidated_suites/provider_contracts/luma_agents_secret_contract.rs"]
mod luma_agents_secret_contract;
#[path = "consolidated_suites/provider_contracts/luma_mcp_management_contract.rs"]
mod luma_mcp_management_contract;
#[path = "consolidated_suites/provider_contracts/model_capacity_contract.rs"]
mod model_capacity_contract;
#[path = "consolidated_suites/provider_contracts/network_timeout_contract.rs"]
mod network_timeout_contract;
#[path = "consolidated_suites/provider_contracts/nous_portal_material_contract.rs"]
mod nous_portal_material_contract;
#[path = "consolidated_suites/provider_contracts/nous_portal_product_contract.rs"]
mod nous_portal_product_contract;
#[path = "consolidated_suites/provider_contracts/observability_contract.rs"]
mod observability_contract;
#[path = "consolidated_suites/provider_contracts/openai_custom_tool_stream_contract.rs"]
mod openai_custom_tool_stream_contract;
#[path = "consolidated_suites/provider_contracts/openai_request_fidelity_contract.rs"]
mod openai_request_fidelity_contract;
#[path = "consolidated_suites/provider_contracts/openai_tool_response_contract.rs"]
mod openai_tool_response_contract;
#[path = "consolidated_suites/provider_contracts/perplexity_web_dispatch_gate.rs"]
mod perplexity_web_dispatch_gate;
#[path = "consolidated_suites/provider_contracts/perplexity_web_feature_contract.rs"]
mod perplexity_web_feature_contract;
#[path = "consolidated_suites/provider_contracts/perplexity_web_material_contract.rs"]
mod perplexity_web_material_contract;
#[path = "consolidated_suites/provider_contracts/perplexity_web_material_roundtrip.rs"]
mod perplexity_web_material_roundtrip;
#[path = "consolidated_suites/provider_contracts/protocol_family_contract.rs"]
mod protocol_family_contract;
#[path = "consolidated_suites/provider_contracts/protocol_live_bridge_contract.rs"]
mod protocol_live_bridge_contract;
#[path = "consolidated_suites/provider_contracts/provider_discovery_inventory_contract.rs"]
mod provider_discovery_inventory_contract;
#[path = "consolidated_suites/provider_contracts/provider_discovery_sampling_contract.rs"]
mod provider_discovery_sampling_contract;
#[path = "consolidated_suites/provider_contracts/provider_model_mapping_contract.rs"]
mod provider_model_mapping_contract;
#[path = "consolidated_suites/provider_contracts/qwen_web_secret_contract.rs"]
mod qwen_web_secret_contract;
#[path = "consolidated_suites/provider_contracts/qwen_web_typed_contract.rs"]
mod qwen_web_typed_contract;
#[path = "consolidated_suites/provider_contracts/qwen_workspace_config_contract.rs"]
mod qwen_workspace_config_contract;
#[path = "consolidated_suites/provider_contracts/rate_limit_enforcement_contract.rs"]
mod rate_limit_enforcement_contract;
#[path = "consolidated_suites/provider_contracts/reference_stream_contract.rs"]
mod reference_stream_contract;
#[path = "consolidated_suites/provider_contracts/request_budget_contract.rs"]
mod request_budget_contract;
#[path = "consolidated_suites/provider_contracts/request_budget_rejected_channel.rs"]
mod request_budget_rejected_channel;
#[path = "consolidated_suites/provider_contracts/request_budget_wait_contract.rs"]
mod request_budget_wait_contract;
#[path = "consolidated_suites/provider_contracts/request_client_profiles_contract.rs"]
mod request_client_profiles_contract;
#[path = "consolidated_suites/provider_contracts/responses_input_arguments_contract.rs"]
mod responses_input_arguments_contract;
#[path = "consolidated_suites/provider_contracts/responses_native_completion_contract.rs"]
mod responses_native_completion_contract;
#[path = "consolidated_suites/provider_contracts/responses_reasoning_config_contract.rs"]
mod responses_reasoning_config_contract;
#[path = "consolidated_suites/provider_contracts/responses_stream_completeness_contract.rs"]
mod responses_stream_completeness_contract;
#[path = "consolidated_suites/provider_contracts/responses_terminal_error_contract.rs"]
mod responses_terminal_error_contract;
#[path = "consolidated_suites/provider_contracts/retry_wait_budget_contract.rs"]
mod retry_wait_budget_contract;
#[path = "consolidated_suites/provider_contracts/retry_wait_contract.rs"]
mod retry_wait_contract;
#[path = "consolidated_suites/provider_contracts/retry_wait_logging_contract.rs"]
mod retry_wait_logging_contract;
#[path = "consolidated_suites/provider_contracts/site_health_adapters_contract.rs"]
mod site_health_adapters_contract;
#[path = "consolidated_suites/provider_contracts/site_health_contract.rs"]
mod site_health_contract;
#[path = "consolidated_suites/provider_contracts/site_health_fakeip_contract.rs"]
mod site_health_fakeip_contract;
#[path = "consolidated_suites/provider_contracts/site_health_gateway_contract.rs"]
mod site_health_gateway_contract;
#[path = "consolidated_suites/provider_contracts/site_health_official_contract.rs"]
mod site_health_official_contract;
#[path = "consolidated_suites/provider_contracts/site_health_profiles_contract.rs"]
mod site_health_profiles_contract;
#[path = "consolidated_suites/provider_contracts/site_health_revision_compatibility.rs"]
mod site_health_revision_compatibility;
#[path = "consolidated_suites/provider_contracts/site_health_secrets_contract.rs"]
mod site_health_secrets_contract;
#[path = "consolidated_suites/provider_contracts/stream_observation_contract.rs"]
mod stream_observation_contract;
#[path = "consolidated_suites/provider_contracts/upstream_model_pricing_contract.rs"]
mod upstream_model_pricing_contract;
#[path = "consolidated_suites/provider_contracts/workbuddy_codex_context_contract.rs"]
mod workbuddy_codex_context_contract;
#[path = "consolidated_suites/provider_contracts/workbuddy_profile_contract.rs"]
mod workbuddy_profile_contract;
