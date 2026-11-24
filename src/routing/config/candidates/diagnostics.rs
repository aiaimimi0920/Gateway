//! Bounded, secret-free rejection facts from the same snapshot as candidate resolution.
use super::*;

pub const MAX_REJECTIONS: usize = 100;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectedCandidate {
    pub provider_account_id: String,
    pub provider_label: String,
    pub real_credential_ref: Option<String>,
    pub resolved_model: Option<String>,
    pub reason: String,
}

impl RejectedCandidate {
    pub(crate) fn from_candidate(candidate: &RouteCandidate, reason: &str) -> Self {
        Self {
            provider_account_id: candidate.provider_account_id.clone(),
            provider_label: candidate.label.clone(),
            real_credential_ref: candidate
                .provider_credential_id
                .clone()
                .or_else(|| candidate.payload.credential_id.clone()),
            resolved_model: candidate.upstream_model.clone(),
            reason: reason.into(),
        }
    }
}

pub(super) fn collect(
    out: &mut Vec<RejectedCandidate>,
    provider: &CompiledProvider,
    model: Option<&str>,
    allowed: Option<&HashSet<String>>,
    admission: Admission<'_>,
) {
    let translated = model
        .and_then(|m| provider.model_map.get(m))
        .map(String::as_str);
    let targets = model.and_then(|m| provider.model_map_targets.get(m));
    let mut record = |id: &str, reason: &str| {
        if out.len() < MAX_REJECTIONS {
            out.push(RejectedCandidate {
                provider_account_id: provider.id.clone(),
                provider_label: provider.label.clone(),
                real_credential_ref: Some(id.into()),
                resolved_model: translated.or(model).map(str::to_owned),
                reason: reason.into(),
            });
        }
    };
    if provider.uses_provider_default_account() {
        let id = provider_default_account_id(&provider.id);
        if allowed.is_some_and(|ids| !ids.contains(&id)) {
            return;
        }
        if !provider.credential_pool_auth_policy.is_any() {
            record(&id, "credential_auth_policy");
        } else if allowed.is_some()
            && !provider.supported_models.is_empty()
            && model.is_some_and(|m| !model_mapping::provider_supports_model(provider, m))
        {
            record(&id, "model_not_supported");
        } else if !admits(admission, &provider.id, &id, translated.or(model)) {
            record(&id, "temporarily_unavailable");
        }
        return;
    }
    for credential in &provider.credential_pool {
        // Do not leak identities belonging to accounts outside this request's authority.
        if allowed.is_some_and(|ids| !ids.contains(&credential.id)) {
            continue;
        }
        let supports = targets.map_or_else(
            || credential_supports_model(credential, model, translated),
            |targets| {
                targets
                    .iter()
                    .any(|m| credential_supports_model(credential, model, Some(m)))
            },
        );
        let reason = if !credential.enabled {
            Some("account_disabled")
        } else if credential.discovery_required {
            Some("discovery_required")
        } else if credential
            .refresh_config
            .as_ref()
            .is_some_and(|owner| !owner.is_routing_active())
        {
            Some("credential_inactive")
        } else if !supports {
            Some("model_not_supported")
        } else if !provider.credential_pool_auth_policy.allows(credential) {
            Some("credential_auth_policy")
        } else if !targets.map_or_else(
            || {
                admits(
                    admission,
                    &provider.id,
                    &credential.id,
                    translated.or(model),
                )
            },
            |targets| {
                targets.iter().any(|m| {
                    credential_supports_model(credential, model, Some(m))
                        && admits(admission, &provider.id, &credential.id, Some(m))
                })
            },
        ) {
            Some("temporarily_unavailable")
        } else {
            None
        };
        if let Some(reason) = reason {
            record(&credential.id, reason);
        }
    }
}

// Legacy unrestricted model compatibility may admit a candidate that a strict model check rejected.
// Keep only actual exclusions; resolution, not diagnostics, remains the eligibility authority.
pub(super) fn remove_admitted(out: &mut Vec<RejectedCandidate>, candidates: &[RouteCandidate]) {
    out.retain(|rejected| {
        !candidates.iter().any(|candidate| {
            candidate.provider_account_id == rejected.provider_account_id
                && candidate.payload.credential_id.as_deref().map_or_else(
                    || {
                        rejected.real_credential_ref.as_deref()
                            == Some(
                                provider_default_account_id(&candidate.provider_account_id)
                                    .as_str(),
                            )
                    },
                    |id| rejected.real_credential_ref.as_deref() == Some(id),
                )
        })
    });
}
