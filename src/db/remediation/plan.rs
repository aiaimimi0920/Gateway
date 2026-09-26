use super::route_policy_patch::build_tighter_rate_limit;
use super::*;

pub(super) fn build_incident_remediation_plan(
    generated_at: OffsetDateTime,
    incident: GatewayAnalysisAnomalyIncidentView,
    policy: Option<GatewayAnalysisAnomalyPolicyView>,
    route_policy: Option<GatewayRoutePolicyView>,
    incident_context: IncidentSyncContext,
) -> GatewayAnalysisAnomalyIncidentRemediationPlanView {
    let route_policy_id = route_policy
        .as_ref()
        .map(|item| item.id.clone())
        .or_else(|| {
            policy
                .as_ref()
                .and_then(|item| item.route_policy_id.clone())
        })
        .or_else(|| incident.route_policy_id.clone());
    let mut actions = Vec::new();

    if matches!(
        incident.code.as_str(),
        "provider_routing_score_drop"
            | "degraded_provider_route_spike"
            | "saturated_provider_route_spike"
            | "breaker_open_provider_route_detected"
    ) {
        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "provider-routing-review".to_string(),
                title: "Review Provider Routing Degradation".to_string(),
                description: "Inspect provider routing score, degradation reasons, saturation, and breaker-open drift for the linked route policy before traffic keeps concentrating on unhealthy providers.".to_string(),
                category: "provider".to_string(),
                priority: "high".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: false,
                execution_mode: "informational".to_string(),
                default_execution_input: None,
                recommended_changes: Some(if let Some(route_policy) = route_policy.as_ref() {
                    json!({
                        "routePolicyId": route_policy.id,
                        "inspect": [
                            "allowedProviderAccountIds",
                            "providerMaxConcurrentRequests",
                            "providerLoadAwareRoutingEnabled",
                            "circuitBreakerThreshold",
                            "circuitBreakerCooldownSeconds"
                        ]
                    })
                } else {
                    json!({
                        "inspect": ["route_policy", "provider_routing_score", "degradation_reasons"]
                    })
                }),
            },
        );

        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "disable-prestream-fallback".to_string(),
                title: "Disable Pre-stream Fallback".to_string(),
                description: "Stop broad pre-stream fallback while unhealthy providers are being isolated, so routing no longer fan-outs across already degraded candidates.".to_string(),
                category: "routing".to_string(),
                priority: "high".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: route_policy_id.is_some(),
                execution_mode: if route_policy_id.is_some() {
                    "route_policy_patch".to_string()
                } else {
                    "informational".to_string()
                },
                default_execution_input: route_policy_id.as_ref().map(|_| {
                    json!({
                        "routePolicyPatch": {
                            "preStreamFallbackEnabled": false
                        }
                    })
                }),
                recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                    json!({
                        "routePolicyId": route_policy_id,
                        "patch": {
                            "preStreamFallbackEnabled": false
                        }
                    })
                }),
            },
        );

        let provider_max_concurrent_requests = route_policy
            .as_ref()
            .and_then(|item| item.config.provider_max_concurrent_requests)
            .map(|value| value.saturating_sub(1).max(1))
            .unwrap_or(1);
        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "reduce-provider-concurrency".to_string(),
                title: "Reduce Provider Concurrency Cap".to_string(),
                description: "Lower the per-provider concurrency cap so unhealthy providers stop receiving the same level of parallel pressure while routing stabilizes.".to_string(),
                category: "provider".to_string(),
                priority: "high".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: route_policy_id.is_some(),
                execution_mode: if route_policy_id.is_some() {
                    "route_policy_patch".to_string()
                } else {
                    "informational".to_string()
                },
                default_execution_input: route_policy_id.as_ref().map(|_| {
                    json!({
                        "routePolicyPatch": {
                            "providerMaxConcurrentRequests": provider_max_concurrent_requests
                        }
                    })
                }),
                recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                    json!({
                        "routePolicyId": route_policy_id,
                        "patch": {
                            "providerMaxConcurrentRequests": provider_max_concurrent_requests
                        }
                    })
                }),
            },
        );

        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "provider-isolation".to_string(),
                title: "Isolate Degraded Providers".to_string(),
                description: "Temporarily narrow the provider allowlist so degraded, saturated, or breaker-open providers are removed from the active route pool.".to_string(),
                category: "provider".to_string(),
                priority: "high".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: route_policy_id.is_some(),
                execution_mode: if route_policy_id.is_some() {
                    "route_policy_patch".to_string()
                } else {
                    "informational".to_string()
                },
                default_execution_input: route_policy.as_ref().and_then(|route_policy| {
                    route_policy
                        .config
                        .allowed_provider_account_ids
                        .as_ref()
                        .and_then(|items| items.first().cloned())
                        .map(|provider_account_id| {
                            json!({
                                "routePolicyPatch": {
                                    "allowedProviderAccountIds": [provider_account_id]
                                }
                            })
                        })
                }),
                recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                    json!({
                        "routePolicyId": route_policy_id,
                        "mode": "narrow_provider_pool"
                    })
                }),
            },
        );
    }

    if matches!(
        incident.code.as_str(),
        "rate_limit_request_spike"
            | "rate_limit_code_concentration"
            | "rate_limit_project_hotspot"
            | "rate_limit_api_key_hotspot"
            | "rate_limit_model_hotspot"
            | "rate_limit_endpoint_hotspot"
    ) {
        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "rate-limit-hotspot-review".to_string(),
                title: "Review Rate-limit Hotspot".to_string(),
                description: "Inspect hotspot entity key, preflight rate limits, requested model, api key, and endpoint distribution before auto-remediation keeps tightening the same route policy.".to_string(),
                category: "routing".to_string(),
                priority: "high".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: false,
                execution_mode: "informational".to_string(),
                default_execution_input: None,
                recommended_changes: Some(json!({
                    "focusCode": incident.code,
                    "focusValue": incident.latest_value,
                    "inspect": ["hotspot_entity_key", "preflight_rate_limits", "requested_model", "api_key_id", "endpoint_kind"]
                })),
            },
        );

        if route_policy_id.is_some()
            && matches!(
                incident.code.as_str(),
                "rate_limit_request_spike"
                    | "rate_limit_code_concentration"
                    | "rate_limit_project_hotspot"
            )
        {
            let current_project_rate_limit = route_policy.as_ref().and_then(|item| {
                match (
                    item.config.rate_limit_window_seconds,
                    item.config.rate_limit_max_requests,
                ) {
                    (Some(window_seconds), Some(max_requests)) => {
                        Some(super::GatewayRateLimitDefinition {
                            window_seconds,
                            max_requests,
                        })
                    }
                    _ => None,
                }
            });
            let target_project_rate_limit = build_tighter_rate_limit(
                current_project_rate_limit.as_ref(),
                &super::GatewayRateLimitDefinition {
                    window_seconds: 60,
                    max_requests: 30,
                },
            );
            push_action(
                &mut actions,
                GatewayAnalysisAnomalyIncidentRemediationActionView {
                    action_key: "tighten-project-rate-limit".to_string(),
                    title: "Tighten Project Rate Limit".to_string(),
                    description: "Apply a stricter route-level request cap so project-wide hotspot pressure stops spilling into the same route policy.".to_string(),
                    category: "routing".to_string(),
                    priority: "high".to_string(),
                    route_policy_id: route_policy_id.clone(),
                    executable: true,
                    execution_mode: "route_policy_patch".to_string(),
                    default_execution_input: Some(json!({
                        "routePolicyPatch": {
                            "projectRateLimit": target_project_rate_limit
                        }
                    })),
                    recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                        json!({
                            "routePolicyId": route_policy_id,
                            "patch": {
                                "projectRateLimit": target_project_rate_limit
                            }
                        })
                    }),
                },
            );
        }

        if route_policy_id.is_some() && incident.code == "rate_limit_api_key_hotspot" {
            let target_api_key_rate_limit = build_tighter_rate_limit(
                route_policy
                    .as_ref()
                    .and_then(|item| item.config.api_key_rate_limit.as_ref()),
                &super::GatewayRateLimitDefinition {
                    window_seconds: 60,
                    max_requests: 20,
                },
            );
            push_action(
                &mut actions,
                GatewayAnalysisAnomalyIncidentRemediationActionView {
                    action_key: "tighten-api-key-rate-limit".to_string(),
                    title: "Tighten Per-key Rate Limit".to_string(),
                    description: "Lower the route policy's per-key rate limit so one hot key stops dominating the request budget.".to_string(),
                    category: "routing".to_string(),
                    priority: "high".to_string(),
                    route_policy_id: route_policy_id.clone(),
                    executable: true,
                    execution_mode: "route_policy_patch".to_string(),
                    default_execution_input: Some(json!({
                        "routePolicyPatch": {
                            "apiKeyRateLimit": target_api_key_rate_limit
                        }
                    })),
                    recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                        json!({
                            "routePolicyId": route_policy_id,
                            "patch": {
                                "apiKeyRateLimit": target_api_key_rate_limit
                            }
                        })
                    }),
                },
            );
        }

        if route_policy_id.is_some()
            && incident.code == "rate_limit_model_hotspot"
            && incident_context.entity_key.is_some()
        {
            let entity_key = incident_context.entity_key.clone().unwrap_or_default();
            let target_model_rate_limit = build_tighter_rate_limit(
                route_policy
                    .as_ref()
                    .and_then(|item| item.config.model_rate_limits.as_ref())
                    .and_then(|limits| limits.get(&entity_key)),
                &super::GatewayRateLimitDefinition {
                    window_seconds: 60,
                    max_requests: 20,
                },
            );
            push_action(
                &mut actions,
                GatewayAnalysisAnomalyIncidentRemediationActionView {
                    action_key: "tighten-model-rate-limit".to_string(),
                    title: "Tighten Hot Model Rate Limit".to_string(),
                    description: "Apply a stricter model-specific limit for the dominant requested model so hotspot traffic does not keep concentrating on the same model.".to_string(),
                    category: "routing".to_string(),
                    priority: "high".to_string(),
                    route_policy_id: route_policy_id.clone(),
                    executable: true,
                    execution_mode: "route_policy_patch".to_string(),
                    default_execution_input: Some(json!({
                        "routePolicyPatch": {
                            "modelRateLimitKey": entity_key,
                            "modelRateLimit": target_model_rate_limit
                        }
                    })),
                    recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                        json!({
                            "routePolicyId": route_policy_id,
                            "patch": {
                                "modelRateLimitKey": entity_key,
                                "modelRateLimit": target_model_rate_limit
                            },
                            "snapshotId": incident_context.snapshot_id
                        })
                    }),
                },
            );
        }

        if route_policy_id.is_some()
            && incident.code == "rate_limit_endpoint_hotspot"
            && incident_context.entity_key.is_some()
        {
            let entity_key = incident_context.entity_key.clone().unwrap_or_default();
            let target_endpoint_rate_limit = build_tighter_rate_limit(
                route_policy
                    .as_ref()
                    .and_then(|item| item.config.endpoint_rate_limits.as_ref())
                    .and_then(|limits| limits.get(&entity_key.to_lowercase())),
                &super::GatewayRateLimitDefinition {
                    window_seconds: 60,
                    max_requests: 20,
                },
            );
            push_action(
                &mut actions,
                GatewayAnalysisAnomalyIncidentRemediationActionView {
                    action_key: "tighten-endpoint-rate-limit".to_string(),
                    title: "Tighten Hot Endpoint Rate Limit".to_string(),
                    description: "Apply a stricter endpoint-specific rate limit for the dominant public endpoint so hotspot traffic stops collapsing onto the same call surface.".to_string(),
                    category: "routing".to_string(),
                    priority: "high".to_string(),
                    route_policy_id: route_policy_id.clone(),
                    executable: true,
                    execution_mode: "route_policy_patch".to_string(),
                    default_execution_input: Some(json!({
                        "routePolicyPatch": {
                            "endpointRateLimitKey": entity_key,
                            "endpointRateLimit": target_endpoint_rate_limit
                        }
                    })),
                    recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                        json!({
                            "routePolicyId": route_policy_id,
                            "patch": {
                                "endpointRateLimitKey": entity_key,
                                "endpointRateLimit": target_endpoint_rate_limit
                            },
                            "snapshotId": incident_context.snapshot_id
                        })
                    }),
                },
            );
        }
    }

    if incident.escalation_status == "escalated" {
        let incident_owner_user_id = incident.owner_user_id.clone();
        let incident_follow_up_status = incident.follow_up_status.clone();
        let incident_latest_note = incident.latest_note.clone();
        let incident_resolution_note = incident.resolution_note.clone();
        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "owner-followup".to_string(),
                title: "Drive Owner Follow-up".to_string(),
                description: "This incident is escalated. Confirm the assigned owner, follow-up status, and remediation note so it does not stay as an unowned escalated signal.".to_string(),
                category: "manual".to_string(),
                priority: "high".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: true,
                execution_mode: "incident_follow_up".to_string(),
                default_execution_input: Some(json!({
                    "incidentFollowUp": {
                        "ownerUserId": incident_owner_user_id,
                        "followUpStatus": if incident_follow_up_status == "pending" {
                            "investigating"
                        } else {
                            incident_follow_up_status.as_str()
                        },
                        "note": incident_latest_note,
                        "resolutionNote": incident_resolution_note
                    }
                })),
                recommended_changes: Some(json!({
                    "ownerUserId": incident.owner_user_id.clone(),
                    "followUpStatus": incident.follow_up_status.clone()
                })),
            },
        );
    }

    if route_policy_id.is_none() {
        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "bind-route-policy".to_string(),
                title: "Bind a Route Policy to the Anomaly Policy".to_string(),
                description: "Link this anomaly policy to a concrete route policy so future anomalies can map directly to routing remediation rather than staying detached.".to_string(),
                category: "routing".to_string(),
                priority: "medium".to_string(),
                route_policy_id: None,
                executable: false,
                execution_mode: "informational".to_string(),
                default_execution_input: None,
                recommended_changes: Some(json!({ "field": "routePolicyId" })),
            },
        );
    }

    if actions.is_empty() {
        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "manual-triage".to_string(),
                title: "Perform Manual Triage".to_string(),
                description: "No specialized remediation rule matched. Review the incident artifacts, trend deltas, and route trace before deciding whether to tune routing, providers, or prompt behavior.".to_string(),
                category: "manual".to_string(),
                priority: "medium".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: false,
                execution_mode: "informational".to_string(),
                default_execution_input: None,
                recommended_changes: None,
            },
        );
    }

    GatewayAnalysisAnomalyIncidentRemediationPlanView {
        generated_at: format_timestamp(generated_at),
        incident,
        policy: policy
            .as_ref()
            .and_then(|value| serde_json::to_value(value).ok()),
        route_policy,
        overview: if actions
            .iter()
            .any(|item| item.action_key == "owner-followup")
        {
            "Escalated incident. Prioritize routing and ownership actions first.".to_string()
        } else {
            "Actionable remediation suggestions based on anomaly code, policy scope, and routing context.".to_string()
        },
        actions,
    }
}

fn push_action(
    actions: &mut Vec<GatewayAnalysisAnomalyIncidentRemediationActionView>,
    action: GatewayAnalysisAnomalyIncidentRemediationActionView,
) {
    if !actions
        .iter()
        .any(|item| item.action_key == action.action_key)
    {
        actions.push(action);
    }
}
