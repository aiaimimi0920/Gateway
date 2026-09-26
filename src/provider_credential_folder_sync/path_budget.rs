//! Bounded ownership for watcher deletion intent.

use std::collections::HashSet;

pub(super) const WATCHER_DELETED_PATH_BYTES: usize = 8 * 1024 * 1024;
const PATH_ENTRY_OVERHEAD: usize = 128;

#[derive(Debug, Clone, Default)]
pub(super) struct BoundedPathSet {
    paths: HashSet<String>,
    retained_bytes: usize,
    overflowed: bool,
}

impl BoundedPathSet {
    pub(super) fn insert(&mut self, path: String) {
        if self.overflowed || self.paths.contains(&path) {
            return;
        }
        let weight = path.capacity().saturating_add(PATH_ENTRY_OVERHEAD);
        if self.retained_bytes.saturating_add(weight) > WATCHER_DELETED_PATH_BYTES {
            self.paths.clear();
            self.retained_bytes = 0;
            self.overflowed = true;
            return;
        }
        self.retained_bytes = self.retained_bytes.saturating_add(weight);
        self.paths.insert(path);
    }

    pub(super) fn extend<I>(&mut self, paths: I)
    where
        I: IntoIterator<Item = String>,
    {
        for path in paths {
            self.insert(path);
            if self.overflowed {
                break;
            }
        }
    }

    pub(super) fn merge(&mut self, other: Self) {
        if self.overflowed || other.overflowed {
            self.clear();
            self.overflowed = true;
            return;
        }
        self.extend(other.paths);
    }

    pub(super) fn clear(&mut self) {
        self.paths.clear();
        self.retained_bytes = 0;
        self.overflowed = false;
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub(super) fn overflowed(&self) -> bool {
        self.overflowed
    }

    pub(super) fn as_set(&self) -> &HashSet<String> {
        &self.paths
    }
}

impl FromIterator<String> for BoundedPathSet {
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Self {
        let mut paths = Self::default();
        paths.extend(iter);
        paths
    }
}

impl IntoIterator for BoundedPathSet {
    type Item = String;
    type IntoIter = std::collections::hash_set::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        self.paths.into_iter()
    }
}

impl PartialEq<HashSet<String>> for BoundedPathSet {
    fn eq(&self, other: &HashSet<String>) -> bool {
        !self.overflowed && &self.paths == other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_paths_charge_once() {
        let mut paths = BoundedPathSet::default();
        paths.insert("provider/a.json".into());
        paths.insert("provider/a.json".into());
        assert_eq!(paths.as_set().len(), 1);
        assert!(!paths.overflowed());
    }

    #[test]
    fn overflow_clears_partial_deletion_intent() {
        let mut paths = BoundedPathSet::default();
        paths.insert("provider/kept.json".into());
        paths.insert("x".repeat(WATCHER_DELETED_PATH_BYTES));
        assert!(paths.overflowed());
        assert!(paths.is_empty());
        assert_eq!(paths.as_set().len(), 0);
    }

    #[test]
    fn merge_propagates_overflow_without_retaining_paths() {
        let mut paths = BoundedPathSet::default();
        let mut overflowed = BoundedPathSet::default();
        overflowed.insert("x".repeat(WATCHER_DELETED_PATH_BYTES));
        paths.insert("provider/a.json".into());
        paths.merge(overflowed);
        assert!(paths.overflowed());
        assert!(paths.is_empty());
    }
}
