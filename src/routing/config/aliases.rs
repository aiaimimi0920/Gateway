//! Glob matching and deterministic model alias normalization/resolution.

use super::*;

/// Returns `true` if `model` matches `pattern`.
///
/// Pattern rules (evaluated in order):
/// 1. `"*"` — matches everything.
/// 2. If the pattern ends with `"*"` → prefix match on `pattern[..len-1]`.
/// 3. If the pattern starts with `"*"` → suffix match on `pattern[1..]`.
/// 4. Otherwise → exact match.
pub(super) fn glob_match(pattern: &str, model: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return model.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return model.ends_with(suffix);
    }
    pattern == model
}

pub(crate) fn normalized_alias_conflicts(
    aliases: &HashMap<String, String>,
) -> Vec<(String, String, String, String)> {
    let mut source: Vec<(&String, &String)> = aliases.iter().collect();
    source.sort_by(|left, right| left.0.cmp(right.0));
    let mut seen = HashMap::<String, (String, String)>::new();
    let mut conflicts = Vec::new();
    for (key, target) in source {
        let normalized = normalize_model_name(key);
        if let Some((first_key, first_target)) = seen.get(&normalized) {
            if first_target != target {
                conflicts.push((
                    normalized,
                    first_key.clone(),
                    key.clone(),
                    format!("{first_target} vs {target}"),
                ));
            }
        } else {
            seen.insert(normalized, (key.clone(), target.clone()));
        }
    }
    conflicts
}

pub(super) fn normalized_alias_entries(
    aliases: &HashMap<String, String>,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    if let Some((normalized, first_key, second_key, targets)) =
        normalized_alias_conflicts(aliases).into_iter().next()
    {
        return Err(anyhow::anyhow!(
            "normalized alias collision for '{normalized}' between '{first_key}' and '{second_key}' ({targets})"
        ));
    }

    let mut entries = HashMap::<String, String>::new();
    for (key, target) in aliases {
        entries.insert(normalize_model_name(key), target.clone());
    }
    let mut entries: Vec<(String, String)> = entries.into_iter().collect();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(entries)
}

pub(super) fn resolve_alias_inner(guard: &RouteConfigInner, model: &str) -> Option<String> {
    if let Some(target) = guard.aliases.get(model) {
        return Some(target.clone());
    }

    if let Some(target) = resolve_legacy_search_route_model_alias(model) {
        return Some(target.to_string());
    }

    let norm_input = normalize_model_name(model);
    if let Some(target) = guard.normalized_aliases.get(&norm_input) {
        return Some(target.clone());
    }

    if norm_input.len() >= 3 {
        let keys = &guard.sorted_normalized_keys;
        let start = keys.partition_point(|(k, _)| k.as_str() < norm_input.as_str());
        let mut best: Option<&str> = None;
        for (k, v) in &keys[start..] {
            if k.starts_with(&norm_input) {
                if best.is_none() {
                    best = Some(v.as_str());
                }
            } else {
                break;
            }
        }
        if let Some(target) = best {
            return Some(target.to_string());
        }
    }

    None
}
