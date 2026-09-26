use super::associations::build_fallback_priority_label;
use super::cost_hints::build_provider_static_pricing_coverage;
use super::identity::{provider_is_hidden_from_operator_inventory, source_profile_from_account};
use super::inventory::build_inventory_summary;
use super::pricing::{build_gateway_price_rate, estimate_observed_cost_micros};
use super::pricing_editor::resolve_provider_model_market_rate;
use super::*;
use serde_json::json;

#[test]
fn pricing_fields_support_direct_and_extra_body_values() {
    let payload = json!({
        "staticInputMicrosPer1kTokens": 300,
        "extraBody": {
            "platformQuoteOutputMicrosPer1kTokens": "420"
        }
    });

    let static_rate = build_gateway_price_rate(&payload, PriceMode::Static);
    let quote_rate = build_gateway_price_rate(&payload, PriceMode::Quote);

    assert_eq!(static_rate.prompt_micros_per_1k_tokens, Some(300));
    assert_eq!(static_rate.completion_micros_per_1k_tokens, None);
    assert_eq!(quote_rate.completion_micros_per_1k_tokens, Some(420));
    assert!(quote_rate.configured);
}

#[test]
fn observed_cost_uses_prompt_and_completion_rates() {
    let aggregate = GatewayUsageAggregate {
        request_count: 1,
        failure_count: 0,
        recent_request_count_10m: 0,
        recent_failure_count_10m: 0,
        input_tokens: 2500,
        output_tokens: 1000,
        thinking_tokens: 0,
        cached_tokens: 0,
        prompt_tokens: 2500,
        completion_tokens: 1000,
        total_tokens: 3500,
        last_request_at: None,
    };
    let rate = GatewayPriceRateView {
        prompt_micros_per_1k_tokens: Some(1000),
        completion_micros_per_1k_tokens: Some(2000),
        currency: "USD".to_string(),
        configured: true,
        source: "payload".to_string(),
    };

    assert_eq!(estimate_observed_cost_micros(&aggregate, &rate), Some(4500));
}

#[test]
fn model_static_pricing_coverage_uses_model_pricing_map() {
    let provider = GatewayProviderAccountView {
        id: "prov-openai".to_string(),
        label: "OpenAI".to_string(),
        service_provider_key: "openai".to_string(),
        service_provider_label: "OpenAI".to_string(),
        adapter: "openai_compatible".to_string(),
        protocol_family: "openai".to_string(),
        protocol_profile: "openai".to_string(),
        status: "active".to_string(),
        source_kind: Some("official_vendor_api".to_string()),
        aggregator_api_mode: None,
        web_reverse_access_mode: None,
        source_notes: None,
        execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
        endpoint_execution_modes: None,
        payload: json!({
        "modelPricing": {
            "gpt-5.4": {
                "staticInputMicrosPer1kTokens": 2500,
                "staticOutputMicrosPer1kTokens": 15000
            },
            "gpt-5.4-mini": {
                "staticInputMicrosPer1kTokens": 750,
                "staticOutputMicrosPer1kTokens": 4500
            }
        }
        }),
        storage_mode: "inline".to_string(),
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: "2026-04-13T00:00:00Z".to_string(),
        updated_at: "2026-04-13T00:00:00Z".to_string(),
    };

    let fallback_rate = build_gateway_price_rate(&provider.payload, PriceMode::Static);
    let coverage = build_provider_static_pricing_coverage(
        &provider,
        &[
            "gpt-5.4".to_string(),
            "gpt-5.4-mini".to_string(),
            "gpt-5.3-codex".to_string(),
        ],
        &fallback_rate,
    );

    assert_eq!(coverage.total_models, 3);
    assert_eq!(coverage.configured_models, 3);
    assert!(coverage.fully_configured);
    assert!(coverage.missing_models.is_empty());
}

#[test]
fn default_market_price_registry_covers_openai_family_models() {
    let provider = GatewayProviderAccountView {
        id: "codex-platform-provider".to_string(),
        label: "Codex Platform".to_string(),
        service_provider_key: "codex_platform".to_string(),
        service_provider_label: "Codex Platform".to_string(),
        adapter: "openai_compatible".to_string(),
        protocol_family: "openai".to_string(),
        protocol_profile: "codex".to_string(),
        status: "active".to_string(),
        source_kind: Some("official_vendor_api".to_string()),
        aggregator_api_mode: None,
        web_reverse_access_mode: None,
        source_notes: None,
        execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
        endpoint_execution_modes: None,
        payload: json!({}),
        storage_mode: "inline".to_string(),
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: "2026-04-13T00:00:00Z".to_string(),
        updated_at: "2026-04-13T00:00:00Z".to_string(),
    };

    let rate = resolve_provider_model_market_rate(
        &CostProviderRef::from_account(&provider),
        "gpt-5.4-mini",
        &HashMap::new(),
        &GatewayPriceRateView {
            prompt_micros_per_1k_tokens: None,
            completion_micros_per_1k_tokens: None,
            currency: "USD".to_string(),
            configured: false,
            source: "unconfigured".to_string(),
        },
    );

    assert_eq!(rate.prompt_micros_per_1k_tokens, Some(750));
    assert_eq!(rate.completion_micros_per_1k_tokens, Some(4500));
    assert_eq!(rate.source, "default_registry");
}

#[test]
fn source_profile_is_marked_derived_when_source_kind_missing() {
    let provider = GatewayProviderAccountView {
        id: "prov-1".to_string(),
        label: "Provider".to_string(),
        service_provider_key: "provider".to_string(),
        service_provider_label: "Provider".to_string(),
        adapter: "openai_compatible".to_string(),
        protocol_family: "openai".to_string(),
        protocol_profile: "openai".to_string(),
        status: "active".to_string(),
        source_kind: None,
        aggregator_api_mode: None,
        web_reverse_access_mode: None,
        source_notes: None,
        execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
        endpoint_execution_modes: None,
        payload: json!({}),
        storage_mode: "inline".to_string(),
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: "2026-04-13T00:00:00Z".to_string(),
        updated_at: "2026-04-13T00:00:00Z".to_string(),
    };

    let source_profile = source_profile_from_account(&provider);
    assert_eq!(source_profile.source_kind, "unknown");
    assert!(source_profile.derived);
}

#[test]
fn fallback_priority_label_uses_provider_priority_order() {
    let label = build_fallback_priority_label(&[
        GatewayModelAssociationProviderLinkView {
            provider_account_id: "prov-1".to_string(),
            label: "Alpha".to_string(),
            adapter: "openai_compatible".to_string(),
            protocol_family: "openai".to_string(),
            status: "active".to_string(),
            source_profile: GatewaySourceProfileView {
                source_kind: "aggregator_api".to_string(),
                aggregator_api_mode: None,
                web_reverse_access_mode: None,
                source_notes: None,
                derived: false,
            },
            upstream_model: Some("gpt-4.1".to_string()),
            priority: 0,
            weight: 1,
            enabled: true,
            default_model: Some("gpt-4.1".to_string()),
        },
        GatewayModelAssociationProviderLinkView {
            provider_account_id: "prov-2".to_string(),
            label: "Beta".to_string(),
            adapter: "anthropic_compatible".to_string(),
            protocol_family: "anthropic".to_string(),
            status: "active".to_string(),
            source_profile: GatewaySourceProfileView {
                source_kind: "official_vendor_api".to_string(),
                aggregator_api_mode: None,
                web_reverse_access_mode: None,
                source_notes: None,
                derived: false,
            },
            upstream_model: Some("claude-sonnet-4".to_string()),
            priority: 1,
            weight: 1,
            enabled: true,
            default_model: Some("claude-sonnet-4".to_string()),
        },
    ]);

    assert_eq!(label, "Alpha(P0) -> Beta(P1)");
}

#[test]
fn operator_inventory_hides_provider_when_payload_flag_enabled() {
    let provider = GatewayProviderAccountView {
        id: "fixture-provider".to_string(),
        label: "Local XML Fallback Fixture".to_string(),
        service_provider_key: "local_xml_fallback_fixture".to_string(),
        service_provider_label: "Local XML Fallback Fixture".to_string(),
        adapter: "openai_compatible".to_string(),
        protocol_family: "openai".to_string(),
        protocol_profile: "openai_compatible_generic".to_string(),
        status: "active".to_string(),
        source_kind: Some("aggregator_api".to_string()),
        aggregator_api_mode: Some("hosted_compute".to_string()),
        web_reverse_access_mode: None,
        source_notes: None,
        execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
        endpoint_execution_modes: None,
        payload: json!({
            "baseUrl": "http://host.docker.internal:42327",
            "defaultModel": "xml-fallback-fixture",
            "hiddenFromOperatorInventory": true
        }),
        storage_mode: "inline".to_string(),
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: "2026-04-18T00:00:00Z".to_string(),
        updated_at: "2026-04-18T00:00:00Z".to_string(),
    };

    assert!(provider_is_hidden_from_operator_inventory(&provider));
}

#[test]
fn operator_inventory_hides_xml_fixture_default_model() {
    let provider = GatewayProviderAccountView {
        id: "fixture-provider".to_string(),
        label: "Fixture Provider".to_string(),
        service_provider_key: "fixture_provider".to_string(),
        service_provider_label: "Fixture Provider".to_string(),
        adapter: "openai_compatible".to_string(),
        protocol_family: "openai".to_string(),
        protocol_profile: "openai_compatible_generic".to_string(),
        status: "active".to_string(),
        source_kind: Some("aggregator_api".to_string()),
        aggregator_api_mode: Some("hosted_compute".to_string()),
        web_reverse_access_mode: None,
        source_notes: Some("internal_fixture:hidden_from_operator_inventory".to_string()),
        execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
        endpoint_execution_modes: None,
        payload: json!({
            "baseUrl": "http://host.docker.internal:42327",
            "defaultModel": "xml-fallback-fixture"
        }),
        storage_mode: "inline".to_string(),
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: "2026-04-18T00:00:00Z".to_string(),
        updated_at: "2026-04-18T00:00:00Z".to_string(),
    };

    assert!(provider_is_hidden_from_operator_inventory(&provider));
}

#[test]
fn inventory_summary_counts_distinct_service_providers_separately_from_surfaces() {
    fn make_entry(
        account_id: &str,
        service_provider_key: &str,
        protocol_family: &str,
        status: &str,
        degraded: bool,
        breaker_open: bool,
    ) -> GatewayProviderInventoryEntryView {
        GatewayProviderInventoryEntryView {
            provider_account: GatewayProviderAccountView {
                id: account_id.to_string(),
                label: format!("{service_provider_key} {protocol_family}"),
                service_provider_key: service_provider_key.to_string(),
                service_provider_label: service_provider_key.to_string(),
                adapter: format!("{protocol_family}_compatible"),
                protocol_family: protocol_family.to_string(),
                protocol_profile: protocol_family.to_string(),
                status: status.to_string(),
                source_kind: Some("official_vendor_api".to_string()),
                aggregator_api_mode: None,
                web_reverse_access_mode: None,
                source_notes: None,
                execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
                endpoint_execution_modes: None,
                payload: json!({}),
                storage_mode: "inline".to_string(),
                cooldown_until: None,
                last_error: None,
                failure_count: 0,
                last_health_check_at: None,
                created_at: "2026-04-19T00:00:00Z".to_string(),
                updated_at: "2026-04-19T00:00:00Z".to_string(),
            },
            provider_health: GatewayProviderHealthView {
                provider_account_id: account_id.to_string(),
                label: account_id.to_string(),
                adapter: format!("{protocol_family}_compatible"),
                protocol_family: protocol_family.to_string(),
                status: status.to_string(),
                cooldown_until: None,
                failure_count: 0,
                last_error: None,
                last_health_check_at: None,
                active_concurrency: 0,
                breaker_open,
                routing_score: 1.0,
                health_weight: 1.0,
                capacity_weight: 1.0,
                degraded,
                saturated: false,
                degradation_reasons: Vec::new(),
            },
            cost_hints: GatewayProviderCostHintsView {
                static_rate: GatewayPriceRateView {
                    prompt_micros_per_1k_tokens: None,
                    completion_micros_per_1k_tokens: None,
                    currency: "USD".to_string(),
                    configured: false,
                    source: "unconfigured".to_string(),
                },
                platform_quote_rate: GatewayPriceRateView {
                    prompt_micros_per_1k_tokens: None,
                    completion_micros_per_1k_tokens: None,
                    currency: "USD".to_string(),
                    configured: false,
                    source: "unconfigured".to_string(),
                },
                static_pricing_coverage: GatewayProviderStaticPricingCoverageView {
                    total_models: 0,
                    configured_models: 0,
                    fully_configured: false,
                    configured_entries: Vec::new(),
                    missing_models: Vec::new(),
                },
                observed_request_count: 0,
                observed_failure_count: 0,
                recent_request_count_10m: 0,
                recent_failure_count_10m: 0,
                observed_prompt_tokens: 0,
                observed_completion_tokens: 0,
                observed_total_tokens: 0,
                observed_cost_micros: None,
                observed_cost_source: "unavailable".to_string(),
                last_request_at: None,
            },
            provider_quota: None,
        }
    }

    let summary = build_inventory_summary(
        &[
            make_entry(
                "xfyun-openai",
                "xfyun_platform",
                "openai",
                "active",
                false,
                false,
            ),
            make_entry(
                "xfyun-anthropic",
                "xfyun_platform",
                "anthropic",
                "active",
                true,
                false,
            ),
            make_entry(
                "codex-openai",
                "codex_platform",
                "openai",
                "active",
                false,
                true,
            ),
        ],
        GatewayCatalogMetadataView {
            provider_account_count: 3,
            model_alias_count: 0,
            route_policy_count: 0,
            fetched_provider_accounts: 3,
            fetched_model_aliases: 0,
            fetched_route_policies: 0,
        },
    );

    assert_eq!(summary.total_providers, 2);
    assert_eq!(summary.total_provider_surfaces, 3);
    assert_eq!(summary.active_providers, 2);
    assert_eq!(summary.active_provider_surfaces, 3);
    assert_eq!(summary.degraded_providers, 1);
    assert_eq!(summary.degraded_provider_surfaces, 1);
    assert_eq!(summary.breaker_open_providers, 1);
    assert_eq!(summary.breaker_open_provider_surfaces, 1);
}
