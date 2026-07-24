use std::collections::HashMap;
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use time::OffsetDateTime;

use super::{ConsoleConfig, RouteConfigPersistence};
use crate::error::GatewayError;

const ADMIN_RECORD_VERSION: u32 = 1;
const ADMIN_FILE_NAME: &str = "admin.json";

#[derive(Clone, Debug)]
pub struct ConsoleAuthRuntime {
    persistence: RouteConfigPersistence,
    env_management_token: Option<String>,
    secret_grant_ttl_secs: u64,
    secret_grants: Arc<Mutex<HashMap<String, SecretGrantRecord>>>,
}

#[derive(Clone, Debug)]
pub struct ConsoleRequestContext {
    client_ip: IpAddr,
    origin_key: String,
    loopback: bool,
}

impl ConsoleRequestContext {
    pub fn loopback() -> Self {
        Self {
            client_ip: IpAddr::from([127, 0, 0, 1]),
            origin_key: "local".to_string(),
            loopback: true,
        }
    }

    pub fn new(client_ip: IpAddr, origin_key: String) -> Self {
        let loopback = client_ip.is_loopback();
        Self {
            client_ip,
            origin_key,
            loopback,
        }
    }

    pub fn client_ip(&self) -> IpAddr {
        self.client_ip
    }

    pub fn origin_key(&self) -> &str {
        &self.origin_key
    }

    pub fn is_loopback(&self) -> bool {
        self.loopback
    }
}

#[derive(Clone, Debug)]
pub struct AuthenticatedConsoleActor {
    token_fingerprint: String,
    secret_access_granted: bool,
}

impl AuthenticatedConsoleActor {
    pub fn token_fingerprint(&self) -> &str {
        &self.token_fingerprint
    }

    pub fn secret_access_granted(&self) -> bool {
        self.secret_access_granted
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleBootstrapStatus {
    pub needs_bootstrap: bool,
    pub management_configured: bool,
    pub environment_override: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleSessionView {
    pub role: &'static str,
    pub capabilities: Vec<String>,
    pub active_revision: Option<String>,
    pub secret_access_granted: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretGrantView {
    pub grant: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminRecord {
    version: u32,
    token_hash: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
struct SecretGrantRecord {
    token_fingerprint: String,
    origin_key: String,
    client_ip: IpAddr,
    expires_at: OffsetDateTime,
}

impl ConsoleAuthRuntime {
    pub fn new(
        console: &ConsoleConfig,
        env_management_token: Option<String>,
    ) -> Result<Self, GatewayError> {
        let persistence = RouteConfigPersistence::new(&console.state_dir, &console.routes_file)
            .map_err(persistence_to_gateway_error)?;
        Ok(Self {
            persistence,
            env_management_token,
            secret_grant_ttl_secs: console.secret_grant_ttl_secs,
            secret_grants: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn admin_path(&self) -> PathBuf {
        self.persistence.state_root().join(ADMIN_FILE_NAME)
    }

    pub fn bootstrap_status(
        &self,
        request: &ConsoleRequestContext,
    ) -> Result<ConsoleBootstrapStatus, GatewayError> {
        let environment_override = self.env_management_token.is_some();
        let admin_present = self.load_admin_record()?.is_some();
        let needs_bootstrap = !environment_override && !admin_present;
        if needs_bootstrap && !request.is_loopback() {
            return Err(forbidden_error(
                "Gateway console bootstrap is only available from loopback clients",
                "console_bootstrap_loopback_only",
            ));
        }
        Ok(ConsoleBootstrapStatus {
            needs_bootstrap,
            management_configured: environment_override || admin_present,
            environment_override,
        })
    }

    pub fn bootstrap(
        &self,
        request: &ConsoleRequestContext,
        token: &str,
    ) -> Result<(), GatewayError> {
        if !request.is_loopback() {
            return Err(forbidden_error(
                "Gateway console bootstrap is only available from loopback clients",
                "console_bootstrap_loopback_only",
            ));
        }
        let token = normalize_token(token)?;
        if self.env_management_token.is_some() || self.load_admin_record()?.is_some() {
            return Err(GatewayError::conflict(
                "Gateway console administrator is already configured",
            )
            .with_code("console_bootstrap_not_needed"));
        }
        let now = OffsetDateTime::now_utc();
        let record = AdminRecord {
            version: ADMIN_RECORD_VERSION,
            token_hash: hash_token(&token)?,
            created_at: now,
            updated_at: now,
        };
        let guard = self
            .persistence
            .try_writer_lock()
            .map_err(persistence_to_gateway_error)?;
        self.persistence
            .atomic_write_json_locked(&guard, &self.admin_path(), &record)
            .map_err(persistence_to_gateway_error)?;
        Ok(())
    }

    pub fn verify_session(
        &self,
        request: &ConsoleRequestContext,
        token: &str,
        active_revision: Option<&str>,
    ) -> Result<ConsoleSessionView, GatewayError> {
        let actor = self.authenticate_management_token(request, token)?;
        Ok(ConsoleSessionView {
            role: "administrator",
            capabilities: vec![
                "route-config:read".to_string(),
                "route-config:write".to_string(),
                "route-config:validate".to_string(),
                "route-config:revisions".to_string(),
            ],
            active_revision: active_revision.map(str::to_string),
            secret_access_granted: actor.secret_access_granted(),
        })
    }

    pub fn confirm_secret_access(
        &self,
        request: &ConsoleRequestContext,
        current_token: &str,
        confirmation_token: &str,
    ) -> Result<SecretGrantView, GatewayError> {
        let actor = self.authenticate_management_token(request, current_token)?;
        let confirmation_token = normalize_token(confirmation_token)?;
        self.verify_plaintext_token(&confirmation_token)?;
        let now = OffsetDateTime::now_utc();
        let expires_at = now + time::Duration::seconds(self.secret_grant_ttl_secs as i64);
        let grant = format!("grant-{}", uuid::Uuid::new_v4());
        self.secret_grants.lock().insert(
            grant.clone(),
            SecretGrantRecord {
                token_fingerprint: actor.token_fingerprint().to_string(),
                origin_key: request.origin_key().to_string(),
                client_ip: request.client_ip(),
                expires_at,
            },
        );
        Ok(SecretGrantView {
            grant,
            expires_at: expires_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| "unknown".to_string()),
        })
    }

    pub fn rotate_token(
        &self,
        request: &ConsoleRequestContext,
        current_token: &str,
        new_token: &str,
    ) -> Result<(), GatewayError> {
        if self.env_management_token.is_some() {
            return Err(GatewayError::bad_request(
                "Gateway console environment override token cannot be rotated from the UI",
            )
            .with_code("console_environment_token_read_only"));
        }
        let current_actor = self.authenticate_management_token(request, current_token)?;
        let existing = self.load_admin_record()?.ok_or_else(|| {
            GatewayError::service_unavailable(
                "Gateway console administrator has not been bootstrapped",
            )
            .with_code("console_bootstrap_required")
        })?;
        let new_token = normalize_token(new_token)?;
        let updated = AdminRecord {
            version: existing.version,
            token_hash: hash_token(&new_token)?,
            created_at: existing.created_at,
            updated_at: OffsetDateTime::now_utc(),
        };
        let guard = self
            .persistence
            .try_writer_lock()
            .map_err(persistence_to_gateway_error)?;
        self.persistence
            .atomic_write_json_locked(&guard, &self.admin_path(), &updated)
            .map_err(persistence_to_gateway_error)?;
        self.revoke_secret_grants(current_actor.token_fingerprint());
        Ok(())
    }

    pub fn logout(&self, request: &ConsoleRequestContext, token: &str) -> Result<(), GatewayError> {
        let actor = self.authenticate_management_token(request, token)?;
        self.revoke_secret_grants(actor.token_fingerprint());
        Ok(())
    }

    pub fn authenticate_management_token(
        &self,
        request: &ConsoleRequestContext,
        token: &str,
    ) -> Result<AuthenticatedConsoleActor, GatewayError> {
        if !request.is_loopback() {
            return Err(forbidden_error(
                "Gateway console remote access is disabled for non-loopback clients",
                "console_remote_access_forbidden",
            ));
        }
        let token = normalize_token(token)?;
        self.verify_plaintext_token(&token)?;
        let token_fingerprint = token_fingerprint(&token);
        let secret_access_granted = self.has_active_secret_grant(
            &token_fingerprint,
            request.origin_key(),
            request.client_ip(),
        );
        Ok(AuthenticatedConsoleActor {
            token_fingerprint,
            secret_access_granted,
        })
    }

    fn verify_plaintext_token(&self, token: &str) -> Result<(), GatewayError> {
        if let Some(expected) = self.env_management_token.as_deref() {
            if expected.as_bytes().ct_eq(token.as_bytes()).into() {
                return Ok(());
            }
            return Err(console_invalid_token_error());
        }
        let Some(record) = self.load_admin_record()? else {
            return Err(GatewayError::service_unavailable(
                "Gateway console administrator has not been bootstrapped",
            )
            .with_code("console_bootstrap_required"));
        };
        let parsed_hash = PasswordHash::new(&record.token_hash).map_err(|error| {
            GatewayError::service_unavailable(format!(
                "Gateway console admin record is invalid: {error}"
            ))
            .with_code("console_admin_record_invalid")
        })?;
        Argon2::default()
            .verify_password(token.as_bytes(), &parsed_hash)
            .map_err(|_| console_invalid_token_error())
    }

    fn load_admin_record(&self) -> Result<Option<AdminRecord>, GatewayError> {
        let path = self.admin_path();
        if !path.exists() {
            return Ok(None);
        }
        self.persistence
            .validate_optional_managed_file(&path)
            .map_err(persistence_to_gateway_error)?;
        let bytes = fs::read(&path).map_err(|error| {
            GatewayError::service_unavailable(format!(
                "Failed to read Gateway console admin record: {error}"
            ))
            .with_code("console_admin_record_unreadable")
        })?;
        let record: AdminRecord = serde_json::from_slice(&bytes).map_err(|error| {
            GatewayError::service_unavailable(format!(
                "Gateway console admin record is invalid: {error}"
            ))
            .with_code("console_admin_record_invalid")
        })?;
        if record.version != ADMIN_RECORD_VERSION || !record.token_hash.starts_with("$argon2id$") {
            return Err(GatewayError::service_unavailable(
                "Gateway console admin record is invalid",
            )
            .with_code("console_admin_record_invalid"));
        }
        Ok(Some(record))
    }

    fn has_active_secret_grant(
        &self,
        token_fingerprint: &str,
        origin_key: &str,
        client_ip: IpAddr,
    ) -> bool {
        let mut grants = self.secret_grants.lock();
        let now = OffsetDateTime::now_utc();
        grants.retain(|_, record| record.expires_at > now);
        grants.values().any(|record| {
            record.token_fingerprint == token_fingerprint
                && record.origin_key == origin_key
                && record.client_ip == client_ip
                && record.expires_at > now
        })
    }

    fn revoke_secret_grants(&self, token_fingerprint: &str) {
        self.secret_grants
            .lock()
            .retain(|_, record| record.token_fingerprint != token_fingerprint);
    }
}

fn normalize_token(token: &str) -> Result<String, GatewayError> {
    let normalized = token.trim();
    if normalized.is_empty() {
        return Err(GatewayError::unauthorized("Management token is required")
            .with_code("console_management_token_invalid"));
    }
    Ok(normalized.to_string())
}

fn hash_token(token: &str) -> Result<String, GatewayError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(token.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| {
            GatewayError::service_unavailable(format!(
                "Failed to hash the Gateway console management token: {error}"
            ))
            .with_code("console_admin_hash_failed")
        })
}

fn token_fingerprint(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn console_invalid_token_error() -> GatewayError {
    GatewayError::unauthorized("Management token is invalid")
        .with_code("console_management_token_invalid")
}

fn forbidden_error(message: impl Into<String>, code: &'static str) -> GatewayError {
    let mut error = GatewayError::unauthorized(message.into()).with_code(code);
    error.http_status = Some(403);
    error
}

fn persistence_to_gateway_error(error: crate::console::PersistenceError) -> GatewayError {
    GatewayError::service_unavailable(error.to_string()).with_code(error.code())
}
