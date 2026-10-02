//! Shared pool capacity and hysteresis policy for direct and queued refill.
use crate::routing::config::ProviderConfigYaml;

pub const DEFAULT_POOL_MAX_SIZE: usize = 100;
pub const DEFAULT_POOL_MIN_SIZE: usize = 1;
pub const MAX_SAFE_POOL_SIZE: u64 = 9_007_199_254_740_991;
/// A transport batch bound, never a limit on the configured pool capacity.
pub(crate) const MAX_REFILL_BATCH: usize = 10_000;

pub fn pool_max_size(provider: &ProviderConfigYaml) -> usize {
    provider.pool_target_size.unwrap_or(DEFAULT_POOL_MAX_SIZE)
}

pub fn pool_min_size(provider: &ProviderConfigYaml) -> usize {
    provider.pool_min_size.unwrap_or(DEFAULT_POOL_MIN_SIZE)
}

pub fn active_credential_count(provider: &ProviderConfigYaml) -> usize {
    if provider.credentials.is_empty() {
        usize::from(!provider.api_key.trim().is_empty() || provider.auth_token.is_some())
    } else {
        provider
            .credentials
            .iter()
            .filter(|item| item.enabled.unwrap_or(true))
            .count()
    }
}

pub fn remaining_capacity(provider: &ProviderConfigYaml) -> usize {
    pool_max_size(provider).saturating_sub(active_credential_count(provider))
}

pub fn normalize_refill_state(provider: &mut ProviderConfigYaml) {
    if !provider.auto_refill_enabled || pool_min_size(provider) == 0 {
        provider.pool_refill_in_progress = false;
    }
}

pub(super) fn refill_cycle_active(
    provider: &ProviderConfigYaml,
    available: usize,
    running: bool,
) -> bool {
    let minimum = pool_min_size(provider);
    provider.auto_refill_enabled
        && minimum > 0
        && available < pool_max_size(provider)
        && (running || provider.pool_refill_in_progress || available < minimum)
}

pub(crate) fn refill_request_count(
    provider: &ProviderConfigYaml,
    available: usize,
    remaining: usize,
) -> usize {
    if refill_cycle_active(provider, available, provider.pool_refill_in_progress) {
        remaining.min(MAX_REFILL_BATCH)
    } else {
        0
    }
}

pub fn validate_pool_capacity(provider: &ProviderConfigYaml) -> Result<(), &'static str> {
    let maximum = pool_max_size(provider);
    let minimum = pool_min_size(provider);
    if maximum == 0 || maximum as u64 > MAX_SAFE_POOL_SIZE {
        return Err("pool_target_size must be a positive safe integer");
    }
    if minimum as u64 > MAX_SAFE_POOL_SIZE || minimum > maximum {
        return Err(
            "pool_min_size must be a nonnegative safe integer no greater than pool_target_size",
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn provider(
        minimum: Option<usize>,
        maximum: Option<usize>,
        count: usize,
    ) -> ProviderConfigYaml {
        serde_json::from_value(serde_json::json!({
            "id": "p", "base_url": "https://example.invalid", "auto_refill_enabled": true,
            "pool_min_size": minimum, "pool_target_size": maximum,
            "credentials": (0..count).map(|i| serde_json::json!({"id": format!("c{i}")})).collect::<Vec<_>>()
        })).unwrap()
    }
    #[test]
    fn defaults_and_legacy_maximum_are_compatible() {
        let default = provider(None, None, 0);
        assert_eq!(pool_min_size(&default), 1);
        assert_eq!(pool_max_size(&default), 100);
        assert_eq!(pool_max_size(&provider(None, Some(42), 0)), 42);
        assert_eq!(
            pool_max_size(&provider(None, Some(2_000_000), 0)),
            2_000_000
        );
    }
    #[test]
    fn starts_strictly_below_minimum_and_continues_to_maximum() {
        assert!(!refill_cycle_active(
            &provider(Some(3), Some(5), 3),
            3,
            false
        ));
        assert!(refill_cycle_active(
            &provider(Some(3), Some(5), 2),
            2,
            false
        ));
        assert!(refill_cycle_active(&provider(Some(3), Some(5), 4), 4, true));
        assert!(!refill_cycle_active(
            &provider(Some(3), Some(5), 5),
            5,
            true
        ));
        assert!(!refill_cycle_active(
            &provider(Some(0), Some(5), 0),
            0,
            true
        ));
        let mut disabled = provider(Some(3), Some(5), 0);
        disabled.auto_refill_enabled = false;
        assert!(!refill_cycle_active(&disabled, 0, true));
        assert_eq!(remaining_capacity(&disabled), 5);
    }
    #[test]
    fn partial_automatic_cycle_survives_restart_and_disable_clears_it() {
        let mut original = provider(Some(2), Some(5), 3);
        original.pool_refill_in_progress = true;
        let mut reloaded: ProviderConfigYaml =
            serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
        assert!(refill_cycle_active(&reloaded, 3, false));
        reloaded.auto_refill_enabled = false;
        normalize_refill_state(&mut reloaded);
        assert!(!reloaded.pool_refill_in_progress);
        reloaded.auto_refill_enabled = true;
        assert!(!refill_cycle_active(&reloaded, 3, false));
        reloaded.pool_refill_in_progress = true;
        reloaded.pool_min_size = Some(0);
        normalize_refill_state(&mut reloaded);
        assert!(!reloaded.pool_refill_in_progress);
    }

    #[test]
    fn pending_verification_pauses_without_losing_the_cycle_and_lowered_max_stops() {
        let mut p = provider(Some(3), Some(5), 2);
        assert_eq!(refill_request_count(&p, 2, 3), 3);
        p.pool_refill_in_progress = true;
        assert_eq!(refill_request_count(&p, 4, 1), 1);
        assert_eq!(refill_request_count(&p, 4, 0), 0);
        normalize_refill_state(&mut p);
        assert!(p.pool_refill_in_progress);
        assert_eq!(refill_request_count(&p, 4, 1), 1);
        p.pool_target_size = Some(4);
        assert_eq!(refill_request_count(&p, 4, 1), 0);
        p.auto_refill_enabled = false;
        assert_eq!(refill_request_count(&p, 0, 4), 0);
    }

    #[test]
    fn validates_safe_capacity_without_an_arbitrary_business_cap() {
        assert!(validate_pool_capacity(&provider(Some(0), Some(1), 0)).is_ok());
        assert!(validate_pool_capacity(&provider(Some(2), Some(1), 0)).is_err());
        assert!(validate_pool_capacity(&provider(None, Some(0), 0)).is_err());
        assert!(validate_pool_capacity(&provider(None, Some(2_000_000), 0)).is_ok());
        #[cfg(target_pointer_width = "64")]
        {
            assert!(
                validate_pool_capacity(&provider(None, Some(MAX_SAFE_POOL_SIZE as usize), 0))
                    .is_ok()
            );
            assert!(validate_pool_capacity(&provider(
                None,
                Some(MAX_SAFE_POOL_SIZE as usize + 1),
                0
            ))
            .is_err());
        }
    }
}
