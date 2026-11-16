//! Local reconciliation never generates keys or performs discovery. Replacement uncertainty hides old capability.
use super::*;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RegistrationState {
    Absent,
    Stored(String),
    Unconfirmed(String),
}
pub(super) struct Snapshot {
    pub state: RegistrationState,
    pub key: Option<Arc<DeviceKey>>,
    pub device: Option<DeviceRegistration>,
}
pub(super) async fn load(
    tx: &mut Transaction<'_, Sqlite>,
    seal: &LocalSecretSeal,
    binding: &str,
    session: &CatalogSession,
) -> Result<Snapshot, GatewayError> {
    let row: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT r.public_key,r.attempt_id,r.sealed,x.attempt_id FROM local_freebuff_registrations r
         LEFT JOIN local_freebuff_registration_replacements x ON x.binding=r.binding WHERE r.binding=?",
    ).bind(binding).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let Some((public, record_id, sealed, replacement)) = row else {
        return Ok(Snapshot {
            state: RegistrationState::Absent,
            key: None,
            device: None,
        });
    };
    for id in std::iter::once(&record_id).chain(replacement.iter()) {
        if id.len() != 36 || uuid::Uuid::parse_str(id).is_err() {
            return Err(unavailable());
        }
    }
    if replacement.is_some() && sealed.is_none() {
        return Err(unavailable());
    }
    let key = device_key(tx, seal, false).await?.ok_or_else(unavailable)?;
    if public != key.public_key() {
        return Err(unavailable());
    }
    let device = sealed
        .as_deref()
        .map(|sealed| restore(seal, binding, &public, sealed, session, key.clone()))
        .transpose()?;
    let state = if let Some(id) = replacement {
        RegistrationState::Unconfirmed(id)
    } else if device.is_some() {
        RegistrationState::Stored(record_id)
    } else {
        RegistrationState::Unconfirmed(record_id)
    };
    Ok(Snapshot {
        state,
        key: Some(key),
        device,
    })
}
impl LocalRuntime {
    pub(crate) async fn freebuff_registration_state(
        &self,
        provider: &str,
        credential: &str,
        session: &CatalogSession,
    ) -> Result<RegistrationState, GatewayError> {
        let mut tx = self.pool.begin().await.map_err(storage_error)?;
        let snapshot = load(
            &mut tx,
            &self.secret_seal,
            &binding(provider, credential, session),
            session,
        )
        .await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(snapshot.state)
    }
    pub(crate) async fn freebuff_registration(
        &self,
        provider: &str,
        credential: &str,
        session: &CatalogSession,
    ) -> Result<Option<DeviceRegistration>, GatewayError> {
        let mut tx = self.pool.begin().await.map_err(storage_error)?;
        let snapshot = load(
            &mut tx,
            &self.secret_seal,
            &binding(provider, credential, session),
            session,
        )
        .await?;
        tx.commit().await.map_err(storage_error)?;
        if matches!(snapshot.state, RegistrationState::Unconfirmed(_)) && snapshot.device.is_some()
        {
            return Err(GatewayError::conflict("FreeBuff replacement registration is unconfirmed; inspect its local state before explicitly retrying. The previous receipt was retained, not used as a fallback.")
                .with_code("freebuff_device_registration_unconfirmed"));
        }
        Ok(snapshot.device)
    }
}
