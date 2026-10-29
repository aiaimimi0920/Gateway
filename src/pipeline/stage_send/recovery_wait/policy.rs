//! Resolve waiting policy through a proven account binding, never a brand-name guess.
use super::identity;
use crate::{
    error::GatewayError,
    pipeline::PipelineContext,
    provider_identity::ProviderAliases,
    request_wait::{RequestWaitMode, RequestWaitPolicy},
    routing::{candidate::RouteCandidate, config::RouteConfigSnapshot},
};

pub(super) fn for_candidate(
    ctx: &PipelineContext,
    snapshot: &RouteConfigSnapshot,
    aliases: &ProviderAliases,
    candidate: &RouteCandidate,
) -> Result<RequestWaitPolicy, GatewayError> {
    let policy = if aliases
        .counterpart(&candidate.provider_account_id)
        .is_some()
    {
        let binding = identity::bind(ctx, snapshot, aliases, candidate)?
            .ok_or_else(crate::access_key_groups::denied)?;
        let mut current = RequestWaitPolicy::for_provider(snapshot, binding.target);
        current.validate()?;
        if binding.renamed {
            let original = RequestWaitPolicy::for_provider(binding.source, binding.source_id);
            original.validate()?;
            // Renaming must not upgrade a bounded/disabled request to a long wait.
            current.mode = match (original.mode, current.mode) {
                (RequestWaitMode::Disabled, _) | (_, RequestWaitMode::Disabled) => {
                    RequestWaitMode::Disabled
                }
                (RequestWaitMode::Bounded, _) | (_, RequestWaitMode::Bounded) => {
                    RequestWaitMode::Bounded
                }
                _ => RequestWaitMode::ClientConnected,
            };
            current.total_timeout_seconds = original
                .total_timeout_seconds
                .into_iter()
                .chain(current.total_timeout_seconds)
                .min();
        }
        current
    } else {
        RequestWaitPolicy::for_provider(snapshot, &candidate.provider_account_id)
    };
    policy.validate()?;
    // Keep the value owned: later config publication cannot mutate this policy.
    Ok(policy)
}
