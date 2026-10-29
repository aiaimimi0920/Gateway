//! Register explicit equivalence without moving live FIFO waiters to another lock.
use super::*;
use crate::provider_identity::ProviderAliases;

impl WaitRegistry {
    pub(crate) fn reconcile_identities(
        &self,
        incoming: &ProviderAliases,
    ) -> Result<(), GatewayError> {
        let conflict = || {
            GatewayError::service_unavailable("Recovery queue identity publication conflicts")
                .with_code("recovery_wait_identity_conflict")
        };
        let mut entries = self.0.lock();
        // Stale readers may publish a subset, never remove reserved names. Validate
        // the entire union before changing any counter or gate; reject retargets.
        let mut pairs: std::collections::BTreeSet<_> = entries
            .identities
            .pairs()
            .map(|(old, new)| (old.to_owned(), new.to_owned()))
            .collect();
        pairs.extend(
            incoming
                .pairs()
                .map(|(old, new)| (old.to_owned(), new.to_owned())),
        );
        let merged = ProviderAliases::new(pairs.into_iter().collect()).map_err(|_| conflict())?;
        for (old, new) in merged.pairs() {
            if entries.providers.contains_key(old) && entries.providers.contains_key(new) {
                // Two independent FIFO locks cannot be safely spliced. A future
                // publication coordinator must prevent this state, not erase it.
                return Err(conflict());
            }
        }
        for (old, new) in merged.pairs() {
            if let Some(owner) = entries.providers.remove(new) {
                entries.providers.insert(old.to_owned(), owner);
            }
        }
        entries.identities = merged;
        Ok(())
    }
}
