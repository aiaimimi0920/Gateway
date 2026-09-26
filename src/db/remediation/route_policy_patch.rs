use super::*;

pub(super) fn build_route_policy_patch(
    action_key: &str,
    route_policy: &GatewayRoutePolicyView,
    input: Option<GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput>,
) -> Result<RoutePolicyPatchSpec, GatewayError> {
    let input = input.unwrap_or_default();
    match action_key {
        "disable-prestream-fallback" => {
            let next = GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                pre_stream_fallback_enabled: Some(false),
                ..Default::default()
            };
            let changed_fields = if route_policy.config.pre_stream_fallback_enabled {
                vec!["preStreamFallbackEnabled".to_string()]
            } else {
                Vec::new()
            };
            Ok(RoutePolicyPatchSpec {
                next,
                changed_fields,
                summary: "Set preStreamFallbackEnabled=false for the linked route policy."
                    .to_string(),
            })
        }
        "reduce-provider-concurrency" => {
            let current_value = route_policy.config.provider_max_concurrent_requests;
            let target_value = input.provider_max_concurrent_requests.unwrap_or_else(|| {
                current_value
                    .map(|value| value.saturating_sub(1).max(1))
                    .unwrap_or(1)
            });
            if target_value < 1 {
                return Err(GatewayError::conflict(
                    "providerMaxConcurrentRequests 必须是大于等于 1 的整数。",
                ));
            }
            if current_value.is_some_and(|value| target_value > value) {
                return Err(GatewayError::conflict(
                    "当前 remediation 只允许下调 providerMaxConcurrentRequests，不允许上调。",
                ));
            }
            Ok(RoutePolicyPatchSpec {
                next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    provider_max_concurrent_requests: Some(target_value),
                    ..Default::default()
                },
                changed_fields: if current_value == Some(target_value) {
                    Vec::new()
                } else {
                    vec!["providerMaxConcurrentRequests".to_string()]
                },
                summary: format!("Set providerMaxConcurrentRequests={target_value}."),
            })
        }
        "provider-isolation" => {
            let next_provider_ids =
                normalize_provider_account_ids(input.allowed_provider_account_ids.as_deref())
                    .ok_or_else(|| {
                        GatewayError::conflict(
                    "provider-isolation remediation 需要显式提供 allowedProviderAccountIds。",
                )
                    })?;
            let current_allowlist = normalize_provider_account_ids(
                route_policy.config.allowed_provider_account_ids.as_deref(),
            );
            if current_allowlist.as_ref().is_some_and(|items| {
                next_provider_ids
                    .iter()
                    .any(|value| !items.iter().any(|item| item == value))
            }) {
                return Err(GatewayError::conflict(
                    "provider-isolation 只能收窄当前 allowlist，不允许引入新的 providerAccountId。",
                ));
            }
            let changed_fields = if current_allowlist.as_ref() == Some(&next_provider_ids) {
                Vec::new()
            } else {
                vec!["allowedProviderAccountIds".to_string()]
            };
            Ok(RoutePolicyPatchSpec {
                next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    allowed_provider_account_ids: Some(next_provider_ids.clone()),
                    ..Default::default()
                },
                changed_fields,
                summary: format!(
                    "Narrow allowedProviderAccountIds to {}.",
                    next_provider_ids.join(", ")
                ),
            })
        }
        "tighten-project-rate-limit" => {
            let current_definition = match (
                route_policy.config.rate_limit_window_seconds,
                route_policy.config.rate_limit_max_requests,
            ) {
                (Some(window_seconds), Some(max_requests)) => Some(GatewayRateLimitDefinition {
                    window_seconds,
                    max_requests,
                }),
                _ => None,
            };
            let next_definition = normalize_rate_limit_patch_definition(
                "projectRateLimit",
                input.project_rate_limit,
                current_definition.clone(),
            )?
            .ok_or_else(|| {
                GatewayError::conflict("tighten-project-rate-limit 需要显式提供 projectRateLimit。")
            })?;
            ensure_tightened_rate_limit(
                "projectRateLimit",
                current_definition.as_ref(),
                &next_definition,
            )?;
            let changed_fields = if route_policy.config.rate_limit_window_seconds
                == Some(next_definition.window_seconds)
                && route_policy.config.rate_limit_max_requests == Some(next_definition.max_requests)
            {
                Vec::new()
            } else {
                vec![
                    "rateLimitWindowSeconds".to_string(),
                    "rateLimitMaxRequests".to_string(),
                ]
            };
            Ok(RoutePolicyPatchSpec {
                next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    project_rate_limit: Some(next_definition.clone()),
                    ..Default::default()
                },
                changed_fields,
                summary: format!(
                    "Set project rate limit to {}/{}s.",
                    next_definition.max_requests, next_definition.window_seconds
                ),
            })
        }
        "tighten-api-key-rate-limit" => {
            let next_definition = normalize_rate_limit_patch_definition(
                "apiKeyRateLimit",
                input.api_key_rate_limit,
                route_policy.config.api_key_rate_limit.clone(),
            )?
            .ok_or_else(|| {
                GatewayError::conflict("tighten-api-key-rate-limit 需要显式提供 apiKeyRateLimit。")
            })?;
            ensure_tightened_rate_limit(
                "apiKeyRateLimit",
                route_policy.config.api_key_rate_limit.as_ref(),
                &next_definition,
            )?;
            let changed_fields =
                if route_policy.config.api_key_rate_limit.as_ref() == Some(&next_definition) {
                    Vec::new()
                } else {
                    vec!["apiKeyRateLimit".to_string()]
                };
            Ok(RoutePolicyPatchSpec {
                next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    api_key_rate_limit: Some(next_definition.clone()),
                    ..Default::default()
                },
                changed_fields,
                summary: format!(
                    "Set apiKeyRateLimit to {}/{}s.",
                    next_definition.max_requests, next_definition.window_seconds
                ),
            })
        }
        "tighten-model-rate-limit" => {
            let key = normalize_scoped_key(
                "routePolicyPatch.modelRateLimitKey",
                input.model_rate_limit_key.as_deref(),
                false,
            )?;
            let current_definition = route_policy
                .config
                .model_rate_limits
                .as_ref()
                .and_then(|items| items.get(&key))
                .cloned();
            let next_definition = normalize_rate_limit_patch_definition(
                &format!("modelRateLimit.{key}"),
                input.model_rate_limit,
                current_definition.clone(),
            )?
            .ok_or_else(|| {
                GatewayError::conflict("tighten-model-rate-limit 需要显式提供 modelRateLimit。")
            })?;
            ensure_tightened_rate_limit(
                &format!("modelRateLimit.{key}"),
                current_definition.as_ref(),
                &next_definition,
            )?;
            let changed_fields = if route_policy
                .config
                .model_rate_limits
                .as_ref()
                .and_then(|items| items.get(&key))
                == Some(&next_definition)
            {
                Vec::new()
            } else {
                vec!["modelRateLimits".to_string()]
            };
            Ok(RoutePolicyPatchSpec {
                next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    model_rate_limit_key: Some(key.clone()),
                    model_rate_limit: Some(next_definition.clone()),
                    ..Default::default()
                },
                changed_fields,
                summary: format!(
                    "Set modelRateLimits[{key}] to {}/{}s.",
                    next_definition.max_requests, next_definition.window_seconds
                ),
            })
        }
        "tighten-endpoint-rate-limit" => {
            let key = normalize_scoped_key(
                "routePolicyPatch.endpointRateLimitKey",
                input.endpoint_rate_limit_key.as_deref(),
                true,
            )?;
            let current_definition = route_policy
                .config
                .endpoint_rate_limits
                .as_ref()
                .and_then(|items| items.get(&key))
                .cloned();
            let next_definition = normalize_rate_limit_patch_definition(
                &format!("endpointRateLimit.{key}"),
                input.endpoint_rate_limit,
                current_definition.clone(),
            )?
            .ok_or_else(|| {
                GatewayError::conflict(
                    "tighten-endpoint-rate-limit 需要显式提供 endpointRateLimit。",
                )
            })?;
            ensure_tightened_rate_limit(
                &format!("endpointRateLimit.{key}"),
                current_definition.as_ref(),
                &next_definition,
            )?;
            let changed_fields = if route_policy
                .config
                .endpoint_rate_limits
                .as_ref()
                .and_then(|items| items.get(&key))
                == Some(&next_definition)
            {
                Vec::new()
            } else {
                vec!["endpointRateLimits".to_string()]
            };
            Ok(RoutePolicyPatchSpec {
                next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    endpoint_rate_limit_key: Some(key.clone()),
                    endpoint_rate_limit: Some(next_definition.clone()),
                    ..Default::default()
                },
                changed_fields,
                summary: format!(
                    "Set endpointRateLimits[{key}] to {}/{}s.",
                    next_definition.max_requests, next_definition.window_seconds
                ),
            })
        }
        other => Err(GatewayError::conflict(format!(
            "当前 remediation actionKey={other} 不支持 route policy 自动执行。"
        ))),
    }
}

pub(super) fn apply_route_policy_patch(
    route_policy: &GatewayRoutePolicyView,
    patch: &RoutePolicyPatchSpec,
) -> GatewayRoutePolicyView {
    let mut updated = route_policy.clone();
    if let Some(value) = patch.next.provider_max_concurrent_requests {
        updated.config.provider_max_concurrent_requests = Some(value);
    }
    if let Some(value) = patch.next.pre_stream_fallback_enabled {
        updated.config.pre_stream_fallback_enabled = value;
    }
    if let Some(ref value) = patch.next.allowed_provider_account_ids {
        updated.config.allowed_provider_account_ids = Some(value.clone());
    }
    if let Some(ref value) = patch.next.project_rate_limit {
        updated.config.rate_limit_window_seconds = Some(value.window_seconds);
        updated.config.rate_limit_max_requests = Some(value.max_requests);
    }
    if let Some(ref value) = patch.next.api_key_rate_limit {
        updated.config.api_key_rate_limit = Some(value.clone());
    }
    if let (Some(key), Some(value)) = (
        patch.next.model_rate_limit_key.as_ref(),
        patch.next.model_rate_limit.as_ref(),
    ) {
        updated
            .config
            .model_rate_limits
            .get_or_insert_with(HashMap::new)
            .insert(key.clone(), value.clone());
    }
    if let (Some(key), Some(value)) = (
        patch.next.endpoint_rate_limit_key.as_ref(),
        patch.next.endpoint_rate_limit.as_ref(),
    ) {
        updated
            .config
            .endpoint_rate_limits
            .get_or_insert_with(HashMap::new)
            .insert(key.to_ascii_lowercase(), value.clone());
    }
    updated
}

pub(super) fn build_tighter_rate_limit(
    current: Option<&GatewayRateLimitDefinition>,
    fallback: &GatewayRateLimitDefinition,
) -> GatewayRateLimitDefinition {
    let Some(current) = current else {
        return fallback.clone();
    };
    if current.window_seconds <= 0 || current.max_requests <= 0 {
        return fallback.clone();
    }
    let reduction = (f64::from(current.max_requests) * 0.2).ceil() as i32;
    GatewayRateLimitDefinition {
        window_seconds: current.window_seconds,
        max_requests: (current.max_requests - reduction.max(1)).max(1),
    }
}

fn normalize_provider_account_ids(values: Option<&[String]>) -> Option<Vec<String>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values.unwrap_or(&[]) {
        let Some(value) = trimmed_owned_ref(value) else {
            continue;
        };
        if seen.insert(value.to_string()) {
            normalized.push(value.to_string());
        }
    }
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn normalize_scoped_key(
    label: &str,
    value: Option<&str>,
    lower_case: bool,
) -> Result<String, GatewayError> {
    let value = trimmed_owned_ref_opt(value)
        .ok_or_else(|| GatewayError::conflict(format!("{label} 不能为空。")))?;
    Ok(if lower_case {
        value.to_ascii_lowercase()
    } else {
        value.to_string()
    })
}

fn normalize_rate_limit_patch_definition(
    label: &str,
    value: Option<GatewayRateLimitDefinition>,
    fallback: Option<GatewayRateLimitDefinition>,
) -> Result<Option<GatewayRateLimitDefinition>, GatewayError> {
    let definition = value.or(fallback);
    let Some(definition) = definition else {
        return Ok(None);
    };
    if definition.window_seconds <= 0 || definition.max_requests <= 0 {
        return Err(GatewayError::conflict(format!(
            "{label} 需要同时提供大于 0 的 windowSeconds 和 maxRequests。"
        )));
    }
    Ok(Some(definition))
}

fn ensure_tightened_rate_limit(
    label: &str,
    current: Option<&GatewayRateLimitDefinition>,
    next: &GatewayRateLimitDefinition,
) -> Result<(), GatewayError> {
    if next.window_seconds <= 0 || next.max_requests <= 0 {
        return Err(GatewayError::conflict(format!(
            "{label} 需要同时提供 windowSeconds 和 maxRequests。"
        )));
    }
    let Some(current) = current else {
        return Ok(());
    };
    if current.window_seconds <= 0 || current.max_requests <= 0 {
        return Ok(());
    }
    if next.max_requests > current.max_requests {
        return Err(GatewayError::conflict(format!(
            "{label} 当前 remediation 只允许下调 maxRequests，不允许上调。"
        )));
    }
    if next.window_seconds < current.window_seconds {
        return Err(GatewayError::conflict(format!(
            "{label} 当前 remediation 只允许保持或放大 windowSeconds，不允许缩短窗口。"
        )));
    }
    Ok(())
}
