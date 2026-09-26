//! Document compilation, effective identities and runtime fingerprints.

use super::*;

/// Convert a `RouteConfigYaml` into a `RouteConfigInner`.
pub(super) fn compile_yaml(config: RouteConfigYaml) -> Result<RouteConfigInner, anyhow::Error> {
    compile_yaml_with_fingerprints(config).map(|(inner, _)| inner)
}

pub(super) fn compile_yaml_with_fingerprints(
    config: RouteConfigYaml,
) -> Result<(RouteConfigInner, HashMap<String, String>), anyhow::Error> {
    // The compiler has always assigned anonymous credentials by effective
    // `{provider_id}-cred-{index}` identity. Materialize that identity before
    // fingerprinting so a legacy raw document (`id: None`) and its first
    // strict console save (`id: Some(generated)`) reuse the same runtime state.
    let config = materialize_credential_ids(config);
    let mut providers = Vec::with_capacity(config.providers.len());
    let mut fingerprints = HashMap::with_capacity(config.providers.len());
    for provider in config.providers {
        let substituted = subst_provider(provider);
        let fingerprint = provider_runtime_fingerprint(&substituted)?;
        let compiled = compile_provider(substituted)?;
        // Duplicate IDs are tolerated by legacy startup for compatibility, but
        // they are intentionally excluded from reuse because identity is
        // ambiguous in that case.
        fingerprints
            .entry(compiled.id.clone())
            .and_modify(|existing| *existing = String::new())
            .or_insert(fingerprint);
        providers.push(compiled);
    }

    let aliases = config.aliases;

    // Pre-compute normalized alias maps at load time (zero per-request allocation).
    // Ambiguous normalized keys are rejected instead of depending on HashMap order.
    let normalized_entries = normalized_alias_entries(&aliases)?;
    let normalized_aliases: HashMap<String, String> = normalized_entries.iter().cloned().collect();
    let sorted_normalized_keys = normalized_entries;

    Ok((
        RouteConfigInner {
            providers,
            model_routes: config.model_routes,
            aliases,
            normalized_aliases,
            sorted_normalized_keys,
        },
        fingerprints,
    ))
}

fn provider_runtime_fingerprint(config: &ProviderConfigYaml) -> Result<String, anyhow::Error> {
    let value = serde_json::to_value(config)?;
    let canonical = canonical_json_value(value);
    let bytes = serde_json::to_vec(&canonical)?;
    let mut digest = Sha256::new();
    digest.update(bytes);
    Ok(hex::encode(digest.finalize()))
}

fn canonical_json_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut sorted = serde_json::Map::new();
            for (key, value) in entries {
                sorted.insert(key, canonical_json_value(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => {
            Value::Array(values.into_iter().map(canonical_json_value).collect())
        }
        value => value,
    }
}

pub(crate) fn compile_route_document(
    document: RouteConfigYaml,
) -> Result<RouteConfigInner, anyhow::Error> {
    compile_yaml(document)
}

pub(crate) fn effective_credential_id<'a>(
    provider_id: &str,
    index: usize,
    credential: &'a ProviderCredentialYaml,
) -> std::borrow::Cow<'a, str> {
    credential
        .id
        .as_deref()
        .map(std::borrow::Cow::Borrowed)
        .unwrap_or_else(|| std::borrow::Cow::Owned(format!("{provider_id}-cred-{index}")))
}

pub(crate) fn provider_default_account_id(provider_id: &str) -> String {
    format!("{provider_id}::default")
}
