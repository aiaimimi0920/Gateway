//! Bounded, explicit provider identity equivalence; never infer identity from a preset.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default)]
pub(crate) struct ProviderAliases(BTreeMap<String, String>);

impl ProviderAliases {
    const MAX_PAIRS: usize = 256;
    const MAX_ID_BYTES: usize = 256;
    // Conservative queue accounting, not an allocator/RSS guarantee: string payloads
    // plus tree slots/pointers, with room for old and replacement snapshots together.
    pub(crate) const WAIT_SNAPSHOT_BYTES: usize =
        2 * Self::MAX_PAIRS * (2 * Self::MAX_ID_BYTES + 1024);

    pub(crate) fn new(pairs: Vec<(String, String)>) -> Result<Self, &'static str> {
        const INVALID: &str = "Invalid provider identity alias registry";
        if pairs.len() > Self::MAX_PAIRS {
            return Err(INVALID);
        }
        let mut aliases = BTreeMap::new();
        let mut targets = BTreeSet::new();
        for (alias, target) in pairs {
            if [&alias, &target].iter().any(|id| {
                id.is_empty()
                    || id.len() > Self::MAX_ID_BYTES
                    || id.trim() != id.as_str()
                    || id.chars().any(char::is_control)
            }) || alias == target
                || !targets.insert(target.clone())
                || aliases
                    .insert(
                        alias.into_boxed_str().into_string(),
                        target.into_boxed_str().into_string(),
                    )
                    .is_some()
            {
                return Err(INVALID);
            }
        }
        // A registry grants identity equivalence. Reject merges and chains rather
        // than accidentally treating independent providers as the same principal.
        if targets.iter().any(|target| aliases.contains_key(target)) {
            return Err(INVALID);
        }
        Ok(Self(aliases))
    }

    pub(crate) fn counterpart(&self, id: &str) -> Option<&str> {
        self.0.get(id).map(String::as_str).or_else(|| {
            self.0
                .iter()
                .find_map(|(old, new)| (new == id).then_some(old.as_str()))
        })
    }

    pub(crate) fn same(&self, left: &str, right: &str) -> bool {
        left == right || self.counterpart(left) == Some(right)
    }

    pub(crate) fn pairs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(old, new)| (old.as_str(), new.as_str()))
    }

    /// Keep a stable process-local owner even after the published name changes.
    pub(crate) fn original<'a>(&'a self, id: &'a str) -> &'a str {
        self.pairs()
            .find_map(|(old, new)| (new == id).then_some(old))
            .unwrap_or(id)
    }

    /// A route snapshot must never schedule both names of one principal.
    pub(crate) fn select_present<'a>(
        &'a self,
        id: &'a str,
        present: impl Fn(&str) -> bool,
    ) -> Result<Option<&'a str>, &'static str> {
        let original = present(id).then_some(id);
        let alternate = self.counterpart(id).filter(|other| present(other));
        if original.is_some() && alternate.is_some() {
            return Err("Provider identity aliases coexist in route snapshot");
        }
        Ok(original.or(alternate))
    }

    pub(crate) fn expand(&self, allowed: &[String]) -> Vec<String> {
        let authorized: BTreeSet<_> = allowed.iter().collect();
        let mut seen = BTreeSet::new();
        let mut result = Vec::new();
        let mut add = |id: &String| {
            if seen.insert(id.clone()) {
                result.push(id.clone());
            }
        };
        for id in allowed {
            add(id);
        }
        for (alias, target) in &self.0 {
            if authorized.contains(alias) || authorized.contains(target) {
                add(alias);
                add(target);
            }
        }
        result
    }
}

#[cfg(test)]
#[path = "provider_identity/tests.rs"]
mod tests;
