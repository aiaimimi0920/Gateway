//! One installation device key; exact account/audience registration receipts live in separate sealed rows.
use super::{secret_seal::LocalSecretSeal, storage_error, LocalRuntime};
use crate::{
    error::GatewayError,
    protocol::freebuff::catalog::{CatalogSession, DeviceKey, DeviceRegistration},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::signature::{Ed25519KeyPair, KeyPair};
use sqlx::{Sqlite, Transaction};
use std::sync::Arc;
mod registration;
mod status;
pub(crate) use registration::{RegistrationIntent, RegistrationPreparation};
pub(crate) use status::RegistrationState;

pub(super) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS local_freebuff_device_keys (
    id INTEGER PRIMARY KEY CHECK (id=1), public_key TEXT NOT NULL UNIQUE, sealed TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS local_freebuff_registrations (
    binding TEXT PRIMARY KEY, public_key TEXT NOT NULL REFERENCES local_freebuff_device_keys(public_key),
    attempt_id TEXT NOT NULL, sealed TEXT);
    CREATE TABLE IF NOT EXISTS local_freebuff_registration_replacements (
    binding TEXT PRIMARY KEY REFERENCES local_freebuff_registrations(binding), attempt_id TEXT NOT NULL);";

fn unavailable() -> GatewayError {
    GatewayError::service_unavailable("FreeBuff device storage is unavailable; restore the original local state and access-key-seal.json. No device key was replaced.")
        .with_code("freebuff_device_secret_unavailable")
}
fn binding(provider: &str, credential: &str, session: &CatalogSession) -> String {
    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    hash.update(b"gateway-freebuff-registration-scope:v1");
    for bytes in [
        provider.as_bytes(),
        credential.as_bytes(),
        &session.registration_scope(),
    ] {
        hash.update(&(bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
    }
    hex::encode(hash.finish())
}
fn device_aad(public: &str) -> String {
    format!("gateway-freebuff-device-key:v1:{public}")
}
fn receipt_aad(binding: &str, public: &str) -> String {
    format!("gateway-freebuff-registration:v1:{binding}:{public}")
}

// Writer transaction serializes key creation across runtime instances; a corrupt row is never absence.
async fn device_key(
    tx: &mut Transaction<'_, Sqlite>,
    seal: &LocalSecretSeal,
    create: bool,
) -> Result<Option<Arc<DeviceKey>>, GatewayError> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT public_key,sealed FROM local_freebuff_device_keys WHERE id=1")
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?;
    if let Some((public, sealed)) = row {
        if public.len() != 43 {
            return Err(unavailable());
        }
        let public_bytes = URL_SAFE_NO_PAD.decode(&public).map_err(|_| unavailable())?;
        let private = seal
            .open_bytes(device_aad(&public).as_bytes(), &sealed, 8192, 4096)
            .map_err(|_| unavailable())?;
        return DeviceKey::from_pkcs8(&private, &public_bytes)
            .map(Arc::new)
            .map(Some)
            .map_err(|_| unavailable());
    }
    if !create {
        return Ok(None);
    }
    let registrations: i64 =
        sqlx::query_scalar("SELECT (SELECT count(*) FROM local_freebuff_registrations) + (SELECT count(*) FROM local_freebuff_registration_replacements)")
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
    if registrations != 0 {
        return Err(unavailable());
    }
    let private = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new())
        .map_err(|_| unavailable())?;
    let pair = Ed25519KeyPair::from_pkcs8(private.as_ref()).map_err(|_| unavailable())?;
    let key = Arc::new(
        DeviceKey::from_pkcs8(private.as_ref(), pair.public_key().as_ref())
            .map_err(|_| unavailable())?,
    );
    let public = key.public_key();
    let sealed = seal
        .seal(device_aad(&public).as_bytes(), private.as_ref(), 4096)
        .map_err(|_| unavailable())?;
    sqlx::query("INSERT INTO local_freebuff_device_keys(id,public_key,sealed) VALUES (1,?,?)")
        .bind(public)
        .bind(sealed)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    Ok(Some(key))
}
fn restore(
    seal: &LocalSecretSeal,
    binding: &str,
    public: &str,
    sealed: &str,
    session: &CatalogSession,
    key: Arc<DeviceKey>,
) -> Result<DeviceRegistration, GatewayError> {
    if public != key.public_key() {
        return Err(unavailable());
    }
    let receipt = seal
        .open_bytes(receipt_aad(binding, public).as_bytes(), sealed, 16384, 8192)
        .map_err(|_| unavailable())?;
    session
        .restore_registration(key, &receipt)
        .map_err(|_| unavailable())
}

#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod tests;
