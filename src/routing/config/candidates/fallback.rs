//! Bounded runtime alternatives: one selection per credential, never an unauthorized pool member.
use super::*;

pub(super) fn provider_candidates(
    provider: &CompiledProvider,
    model: Option<&str>,
    alias: Option<&str>,
    priority: i32,
    allowed: Option<&HashSet<String>>,
    admission: Admission<'_>,
) -> Vec<RouteCandidate> {
    let mut result = Vec::new();
    let mut selected = HashSet::new();
    // Match the global six-send ceiling; catalogue/legacy APIs keep one RR selection.
    let limit = if admission.is_some() { 6 } else { 1 };
    for _ in 0..limit {
        let admit =
            |p: &str, c: &str, m: &str| !selected.contains(c) && admits(admission, p, c, Some(m));
        let Some(mut candidate) = provider_to_candidate(
            provider,
            model,
            alias,
            priority,
            allowed,
            Some(&admit),
            result.is_empty(),
        ) else {
            break;
        };
        let id = candidate
            .payload
            .credential_id
            .clone()
            .unwrap_or_else(|| provider_default_account_id(&provider.id));
        if !selected.insert(id) {
            break;
        }
        // Backups must not multiply the provider's chance of winning the first weighted draw.
        if !result.is_empty() {
            candidate.priority = i32::MIN;
        }
        result.push(candidate);
    }
    result
}
