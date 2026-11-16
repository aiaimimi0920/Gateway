//! Durable prepare-before-send and one-installation OS fence; no automatic retry of uncertain POSTs.
use super::*;
use std::fs::{File, OpenOptions, TryLockError};

pub(crate) enum RegistrationPreparation {
    Existing(RegistrationState),
    Send(RegistrationAttempt),
}
#[derive(Clone, Copy)]
pub(crate) enum RegistrationIntent<'a> {
    Register,
    Retry(&'a str),
    Replace(&'a str),
}
impl RegistrationIntent<'_> {
    // Check before network I/O and again under the installation lock/SQLite writer transaction.
    pub(crate) fn needs_send(self, state: &RegistrationState) -> Result<bool, GatewayError> {
        match (self, state) {
            (Self::Register, RegistrationState::Absent) => Ok(true),
            (Self::Register, _) => Ok(false),
            (Self::Retry(expected), RegistrationState::Unconfirmed(id)) if expected == id => {
                Ok(true)
            }
            (Self::Replace(expected), RegistrationState::Stored(id)) if expected == id => Ok(true),
            _ => Err(conflict("freebuff_device_registration_attempt_changed")),
        }
    }
}
pub(crate) struct RegistrationAttempt {
    _guard: File,
    pool: sqlx::SqlitePool,
    seal: LocalSecretSeal,
    binding: String,
    attempt_id: String,
    scope: [u8; 32],
    key: Arc<DeviceKey>,
    replacement: bool,
}
fn conflict(code: &str) -> GatewayError {
    GatewayError::conflict("FreeBuff registration is busy or unconfirmed. Re-read its state before explicitly authorizing another registration; it is not known to be idempotent.").with_code(code)
}
impl LocalRuntime {
    pub(crate) async fn prepare_freebuff_registration(
        &self,
        provider: &str,
        credential: &str,
        session: &CatalogSession,
        intent: RegistrationIntent<'_>,
    ) -> Result<RegistrationPreparation, GatewayError> {
        if !cfg!(feature = "line-freebuff-web-reverse-api") {
            return Err(conflict("gateway_provider_line_compiled_out"));
        }
        // Fixed name bounds lock files. RAII releases on error/cancellation/crash; never unlink a live lock.
        let path = self
            .lock_directory
            .join("freebuff-device-registration.lock");
        let guard = tokio::task::spawn_blocking(move || {
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(path)
                .map_err(|_| unavailable())?;
            match file.try_lock() {
                Ok(()) => Ok(file),
                Err(TryLockError::WouldBlock) => Err(conflict("freebuff_device_registration_busy")),
                Err(TryLockError::Error(_)) => Err(unavailable()),
            }
        })
        .await
        .map_err(|_| unavailable())??;
        let binding = binding(provider, credential, session);
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let snapshot = status::load(&mut tx, &self.secret_seal, &binding, session).await?;
        if !intent.needs_send(&snapshot.state)? {
            tx.commit().await.map_err(storage_error)?;
            return Ok(RegistrationPreparation::Existing(snapshot.state));
        }
        if snapshot.state == RegistrationState::Absent {
            let count: i64 =
                sqlx::query_scalar("SELECT count(*) FROM local_freebuff_registrations")
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(storage_error)?;
            if count >= 4096 {
                return Err(conflict("freebuff_device_registration_capacity"));
            }
        }
        let key = match snapshot.key {
            Some(key) => key,
            None => device_key(&mut tx, &self.secret_seal, true)
                .await?
                .ok_or_else(unavailable)?,
        };
        let replacement = snapshot.device.is_some();
        // An overlay preserves the old sealed receipt, but does not authorize fallback while uncertain.
        // Every retry consumes the exact current attempt ID before any POST.
        let attempt_id = uuid::Uuid::new_v4().to_string();
        let inserted = if replacement {
            match &snapshot.state {
                RegistrationState::Stored(_) => sqlx::query("INSERT INTO local_freebuff_registration_replacements(binding,attempt_id) VALUES (?,?)")
                    .bind(&binding).bind(&attempt_id).execute(&mut *tx).await,
                RegistrationState::Unconfirmed(old) => sqlx::query("UPDATE local_freebuff_registration_replacements SET attempt_id=? WHERE binding=? AND attempt_id=?")
                    .bind(&attempt_id).bind(&binding).bind(old).execute(&mut *tx).await,
                RegistrationState::Absent => unreachable!("a stored capability has a registration record"),
            }
        } else {
            sqlx::query("INSERT INTO local_freebuff_registrations(binding,public_key,attempt_id,sealed) VALUES (?,?,?,NULL) ON CONFLICT(binding) DO UPDATE SET attempt_id=excluded.attempt_id WHERE sealed IS NULL")
                .bind(&binding).bind(key.public_key()).bind(&attempt_id).execute(&mut *tx).await
        }.map_err(storage_error)?;
        if inserted.rows_affected() != 1 {
            return Err(conflict("freebuff_device_registration_attempt_changed"));
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(RegistrationPreparation::Send(RegistrationAttempt {
            _guard: guard,
            pool: self.pool.clone(),
            seal: self.secret_seal.clone(),
            binding,
            attempt_id,
            scope: session.registration_scope(),
            key,
            replacement,
        }))
    }
}
impl RegistrationAttempt {
    pub(crate) fn key(&self) -> Arc<DeviceKey> {
        self.key.clone()
    }
    pub(crate) async fn complete(
        self,
        session: &CatalogSession,
        device: &DeviceRegistration,
    ) -> Result<String, GatewayError> {
        if session.registration_scope() != self.scope {
            return Err(unavailable());
        }
        let public = self.key.public_key();
        let receipt = session
            .registration_receipt(device)
            .map_err(|_| unavailable())?;
        // Check the returned capability uses this attempt's key, not merely a matching account scope.
        session
            .restore_registration(self.key.clone(), &receipt)
            .map_err(|_| unavailable())?;
        let sealed = self
            .seal
            .seal(
                receipt_aad(&self.binding, &public).as_bytes(),
                &receipt,
                8192,
            )
            .map_err(|_| unavailable())?;
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let result = if self.replacement {
            sqlx::query("UPDATE local_freebuff_registrations SET sealed=?,attempt_id=? WHERE binding=? AND public_key=? AND sealed IS NOT NULL AND EXISTS (SELECT 1 FROM local_freebuff_registration_replacements x WHERE x.binding=local_freebuff_registrations.binding AND x.attempt_id=?)")
                .bind(sealed).bind(&self.attempt_id).bind(&self.binding).bind(public).bind(&self.attempt_id).execute(&mut *tx).await
        } else {
            sqlx::query("UPDATE local_freebuff_registrations SET sealed=? WHERE binding=? AND public_key=? AND attempt_id=? AND sealed IS NULL")
                .bind(sealed).bind(&self.binding).bind(public).bind(&self.attempt_id).execute(&mut *tx).await
        }.map_err(storage_error)?;
        if result.rows_affected() != 1 {
            return Err(conflict("freebuff_device_registration_attempt_changed"));
        }
        if self.replacement {
            let removed = sqlx::query("DELETE FROM local_freebuff_registration_replacements WHERE binding=? AND attempt_id=?")
                .bind(&self.binding).bind(&self.attempt_id).execute(&mut *tx).await.map_err(storage_error)?;
            if removed.rows_affected() != 1 {
                return Err(conflict("freebuff_device_registration_attempt_changed"));
            }
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(self.attempt_id)
    }
}
