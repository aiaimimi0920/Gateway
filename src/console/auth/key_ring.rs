//! Versioned administrator key ring. The first mutation takes ownership from legacy/env auth.
use super::*;
use crate::console::WriterLockGuard;
use serde::{Deserialize, Serialize};

#[path = "key_ring_edit.rs"]
mod editing;
#[path = "key_ring_secret.rs"]
mod secret;

const MAX_KEYS: usize = 16;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KeyRing {
    version: u32,
    revision: String,
    keys: Vec<StoredKey>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredKey {
    id: String,
    name: String,
    token_hash: String,
    created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sealed_token: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagementKeyView {
    id: String,
    name: String,
    created_at: String,
    current: bool,
}

impl KeyRing {
    pub(super) fn verify(
        &self,
        auth: &ConsoleAuthRuntime,
        token: &str,
    ) -> Result<String, GatewayError> {
        let cache_key = format!("{}:{}", self.revision, token_fingerprint(token));
        if let Some(id) = auth.key_verifications.lock().get(&cache_key).cloned() {
            return Ok(id);
        }
        for key in &self.keys {
            let hash = PasswordHash::new(&key.token_hash).map_err(|_| invalid_record())?;
            if Argon2::default()
                .verify_password(token.as_bytes(), &hash)
                .is_ok()
            {
                let mut cache = auth.key_verifications.lock();
                if cache.len() >= 64 {
                    cache.clear();
                }
                cache.insert(cache_key, key.id.clone());
                return Ok(key.id.clone());
            }
        }
        Err(console_invalid_token_error())
    }

    fn views(&self, current: &str) -> Vec<ManagementKeyView> {
        self.keys
            .iter()
            .map(|key| ManagementKeyView {
                id: key.id.clone(),
                name: key.name.clone(),
                created_at: key.created_at.clone(),
                current: key.id == current,
            })
            .collect()
    }
}

fn invalid_record() -> GatewayError {
    GatewayError::service_unavailable("Invalid management key ring")
        .with_code("console_admin_record_invalid")
}

impl ConsoleAuthRuntime {
    pub fn managed_keys_active(&self) -> Result<bool, GatewayError> {
        Ok(self.load_key_ring()?.is_some())
    }

    pub(super) fn load_key_ring(&self) -> Result<Option<KeyRing>, GatewayError> {
        let path = self.admin_path();
        self.persistence
            .validate_optional_managed_file(&path)
            .map_err(persistence_to_gateway_error)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(invalid_record()),
        };
        if bytes.len() > 128 * 1024 {
            return Err(invalid_record());
        }
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| invalid_record())?;
        if value.get("version").and_then(|v| v.as_u64()) == Some(1) {
            return Ok(None);
        }
        let ring: KeyRing = serde_json::from_value(value).map_err(|_| invalid_record())?;
        let mut ids = std::collections::HashSet::new();
        if ring.version != 2
            || ring.revision.is_empty()
            || ring.keys.is_empty()
            || ring.keys.len() > MAX_KEYS
            || ring.keys.iter().any(|key| {
                key.id.is_empty()
                    || !ids.insert(&key.id)
                    || !key.token_hash.starts_with("$argon2id$")
                    || PasswordHash::new(&key.token_hash).is_err()
            })
        {
            return Err(invalid_record());
        }
        Ok(Some(ring))
    }

    fn initial_key_ring(&self) -> Result<KeyRing, GatewayError> {
        let (hash, created) = if let Some(token) = &self.env_management_token {
            (hash_token(token)?, OffsetDateTime::now_utc())
        } else {
            let record = self.load_admin_record()?.ok_or_else(invalid_record)?;
            (record.token_hash, record.created_at)
        };
        Ok(KeyRing {
            version: 2,
            revision: uuid::Uuid::new_v4().to_string(),
            keys: vec![StoredKey {
                id: "initial".into(),
                name: "初始管理密钥".into(),
                token_hash: hash,
                created_at: created
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
                sealed_token: None,
            }],
        })
    }

    pub fn list_management_keys(
        &self,
        request: &ConsoleRequestContext,
        token: &str,
    ) -> Result<Vec<ManagementKeyView>, GatewayError> {
        self.authenticate_management_token(request, token)?;
        if let Some(ring) = self.load_key_ring()? {
            let current = ring.verify(self, token.trim())?;
            Ok(ring.views(&current))
        } else {
            Ok(vec![ManagementKeyView {
                id: "initial".into(),
                name: "初始管理密钥".into(),
                created_at: String::new(),
                current: true,
            }])
        }
    }

    pub fn add_management_key(
        &self,
        request: &ConsoleRequestContext,
        current: &str,
        name: &str,
        token: &str,
    ) -> Result<(), GatewayError> {
        let name = name.trim();
        let token = normalize_token(token)?;
        if name.is_empty()
            || name.chars().count() > 80
            || token.len() > 4096
            || !token.bytes().all(|c| (33..=126).contains(&c))
        {
            return Err(GatewayError::bad_request("Invalid key name or token")
                .with_code("console_key_invalid"));
        }
        let guard = self
            .persistence
            .try_writer_lock()
            .map_err(persistence_to_gateway_error)?;
        self.authenticate_management_token(request, current)?;
        let existing = self.load_key_ring()?;
        let mut ring = match existing {
            Some(ref ring) => ring.clone(),
            None => self.initial_key_ring()?,
        };
        if ring.keys.len() >= MAX_KEYS {
            return Err(
                GatewayError::conflict("At most 16 management keys are allowed")
                    .with_code("console_key_limit"),
            );
        }
        if ring.verify(self, &token).is_ok() {
            return Err(GatewayError::conflict("Management key already exists")
                .with_code("console_key_duplicate"));
        }
        let mut key = StoredKey {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            token_hash: hash_token(&token)?,
            created_at: OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
            sealed_token: None,
        };
        key.sealed_token = Some(self.seal_management_key(&guard, &key, &token)?);
        ring.keys.push(key);
        ring.revision = uuid::Uuid::new_v4().to_string();
        self.save_management_key_ring(&guard, &ring, existing.is_none())?;
        Ok(())
    }

    fn save_management_key_ring(
        &self,
        guard: &WriterLockGuard,
        ring: &KeyRing,
        migrate: bool,
    ) -> Result<(), GatewayError> {
        // Keep a hashed legacy recovery record before the one-way format expansion.
        if migrate {
            if let Some(legacy) = self.load_admin_record()? {
                let backup = self
                    .persistence
                    .state_root()
                    .join("admin-before-keyring.json");
                if !backup.exists() {
                    self.persistence
                        .atomic_write_json_locked(guard, &backup, &legacy)
                        .map_err(persistence_to_gateway_error)?;
                }
            }
        }
        self.persistence
            .atomic_write_json_locked(guard, &self.admin_path(), ring)
            .map_err(persistence_to_gateway_error)?;
        self.key_verifications.lock().clear();
        Ok(())
    }

    pub fn revoke_management_key(
        &self,
        request: &ConsoleRequestContext,
        token: &str,
        id: &str,
    ) -> Result<(), GatewayError> {
        let guard = self
            .persistence
            .try_writer_lock()
            .map_err(persistence_to_gateway_error)?;
        self.authenticate_management_token(request, token)?;
        let mut ring = self.load_key_ring()?.ok_or_else(|| {
            GatewayError::conflict("Cannot revoke the last management key")
                .with_code("console_key_last")
        })?;
        if ring.keys.len() <= 1 {
            return Err(
                GatewayError::conflict("Cannot revoke the last management key")
                    .with_code("console_key_last"),
            );
        }
        if !ring.keys.iter().any(|key| key.id == id) {
            return Err(GatewayError::not_found("Management key not found"));
        }
        ring.keys.retain(|key| key.id != id);
        ring.revision = uuid::Uuid::new_v4().to_string();
        self.persistence
            .atomic_write_json_locked(&guard, &self.admin_path(), &ring)
            .map_err(persistence_to_gateway_error)?;
        self.key_verifications.lock().clear();
        self.secret_grants.lock().clear();
        Ok(())
    }
}
