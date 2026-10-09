//! Multi-target mappings select a model once per candidate, before upstream I/O.
use super::*;
use rand::seq::SliceRandom;

pub(super) fn deserialize_targets<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<HashMap<String, Vec<String>>, D::Error> {
    let mappings = HashMap::<String, Vec<String>>::deserialize(deserializer)?;
    let valid_name = |name: &str| {
        !name.is_empty()
            && name.len() <= 256
            && name.trim() == name
            && !name.contains(['*', '?'])
            && !name.chars().any(char::is_control)
    };
    if mappings.len() > 512
        || mappings.iter().any(|(source, targets)| {
            !valid_name(source)
                || targets.is_empty()
                || targets.len() > 32
                || targets.iter().any(|target| !valid_name(target))
                || targets.iter().collect::<HashSet<_>>().len() != targets.len()
        })
    {
        return Err(serde::de::Error::custom("Invalid model_map_targets: exact names (1..256 bytes), 1..32 unique targets, at most 512 mappings."));
    }
    Ok(mappings)
}

pub(super) fn provider_supports_model(provider: &CompiledProvider, model: &str) -> bool {
    let targets = provider.model_map_targets.get(model);
    provider.supported_models.iter().any(|supported| {
        supported == model
            || provider.model_map.get(model) == Some(supported)
            || targets.is_some_and(|targets| targets.contains(supported))
    }) || targets.is_some_and(|targets| {
        targets
            .iter()
            .any(|target| target_is_available(provider, model, target, None))
    })
}

fn eligible_credentials(
    provider: &CompiledProvider,
    original: &str,
    target: &str,
    allowed: Option<&HashSet<String>>,
) -> Option<Vec<usize>> {
    if provider.credential_pool.is_empty() {
        return (allowed
            .is_none_or(|ids| ids.contains(&provider_default_account_id(&provider.id)))
            && (provider.supported_models.is_empty()
                || provider
                    .supported_models
                    .iter()
                    .any(|m| m == original || m == target)))
        .then(Vec::new);
    }
    let eligible: Vec<_> = provider
        .credential_pool
        .iter()
        .enumerate()
        .filter(|(_, credential)| credential_eligible(credential, original, target, allowed))
        .map(|(index, _)| index)
        .collect();
    (!eligible.is_empty()).then_some(eligible)
}

fn credential_eligible(
    credential: &CompiledCredential,
    original: &str,
    target: &str,
    allowed: Option<&HashSet<String>>,
) -> bool {
    credential.enabled
        && allowed.is_none_or(|ids| ids.contains(&credential.id))
        && (credential.supported_models.is_empty()
            || credential
                .supported_models
                .iter()
                .any(|m| m == original || m == target))
}

fn target_is_available(
    provider: &CompiledProvider,
    original: &str,
    target: &str,
    allowed: Option<&HashSet<String>>,
) -> bool {
    if provider.credential_pool.is_empty() {
        return eligible_credentials(provider, original, target, allowed).is_some();
    }
    provider
        .credential_pool
        .iter()
        .any(|credential| credential_eligible(credential, original, target, allowed))
}

pub(super) fn mapping_available(provider: &CompiledProvider, model: &str) -> bool {
    provider.model_map_targets.get(model).is_none_or(|targets| {
        targets
            .iter()
            .any(|target| target_is_available(provider, model, target, None))
    })
}

pub(super) fn select_mapped_payload(
    provider: &CompiledProvider,
    original: &str,
    allowed: Option<&HashSet<String>>,
) -> Option<(String, ProviderAccountPayload)> {
    let viable: Vec<_> = provider
        .model_map_targets
        .get(original)?
        .iter()
        .filter(|target| target_is_available(provider, original, target, allowed))
        .collect();
    let model = viable.choose(&mut rand::thread_rng())?;
    // Allocate credential indices only for the chosen target, not all alternatives.
    let credentials = eligible_credentials(provider, original, model, allowed)?;
    let payload = if credentials.is_empty() {
        provider.payload.clone()
    } else {
        let counter = provider.credential_counter.fetch_add(1, Ordering::Relaxed);
        candidates::apply_token_override(
            &provider.credential_pool[credentials[counter % credentials.len()]],
        )
    };
    Some(((*model).clone(), payload))
}

/// Console tests retain the exact selected credential; mapping never switches accounts.
pub(crate) fn model_for_probe(
    provider: &CompiledProvider,
    credential_id: &str,
    model: &str,
) -> Option<String> {
    if provider.model_map_targets.contains_key(model) {
        let allowed = HashSet::from([credential_id.to_string()]);
        let targets: Vec<_> = provider
            .model_map_targets
            .get(model)?
            .iter()
            .filter(|target| target_is_available(provider, model, target, Some(&allowed)))
            .collect();
        targets
            .choose(&mut rand::thread_rng())
            .map(|model| (*model).clone())
    } else {
        Some(
            provider
                .model_map
                .get(model)
                .cloned()
                .unwrap_or_else(|| model.to_string()),
        )
    }
}
