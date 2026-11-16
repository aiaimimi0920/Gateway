//! Optional catalog root policy; no guessed budget when an opaque model has no policy.
use serde::{Deserialize, Deserializer};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RootCompactionPolicy {
    pub(crate) cache_expiry_ms: u64,
    pub(crate) cache_expiry_min_tokens: u64,
    #[serde(default, deserialize_with = "optional")]
    pub(crate) max_context_length: Option<u64>,
    /// Builder guidance, not a runtime compactContext field. The future harness must separate it.
    #[serde(default, deserialize_with = "optional")]
    pub(crate) lean: Option<bool>,
}
impl RootCompactionPolicy {
    pub(super) fn valid(&self) -> bool {
        const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
        (1..=MAX_SAFE_INTEGER).contains(&self.cache_expiry_ms)
            && self.cache_expiry_min_tokens <= MAX_SAFE_INTEGER
            && self
                .max_context_length
                .is_none_or(|n| (1..=MAX_SAFE_INTEGER).contains(&n))
    }
}
/// The reference's optional fields may be absent, not null. Serde's default Option accepts both.
pub(super) fn optional<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
