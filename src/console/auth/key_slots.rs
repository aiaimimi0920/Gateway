//! Two UI slots retain hashed keys when disabled. Empty slots never disable authentication.
use super::*;

impl ConsoleAuthRuntime {
    /// Public session marker, not a secret. Valid only for this all-disabled ring revision.
    pub fn passwordless_token(&self) -> Result<Option<String>, GatewayError> {
        Ok(self
            .load_key_ring()?
            .filter(|ring| ring.keys.iter().all(|key| !key.enabled))
            .map(|ring| format!("passwordless:{}", ring.revision)))
    }

    pub(in crate::console::auth) fn is_passwordless_token(
        &self,
        token: &str,
    ) -> Result<bool, GatewayError> {
        Ok(self.passwordless_token()?.as_deref() == Some(token))
    }

    pub fn set_management_key_slot(
        &self,
        request: &ConsoleRequestContext,
        current: &str,
        slot: usize,
        enabled: bool,
        replacement: Option<&str>,
    ) -> Result<(), GatewayError> {
        if slot > 1 {
            return Err(GatewayError::bad_request("Invalid management key slot"));
        }
        let replacement = replacement.map(normalize_token).transpose()?;
        if replacement.as_ref().is_some_and(|token| {
            token.len() > 4096 || !token.bytes().all(|c| (33..=126).contains(&c))
        }) {
            return Err(GatewayError::bad_request("Invalid management key"));
        }
        let guard = self
            .persistence
            .try_writer_lock()
            .map_err(persistence_to_gateway_error)?;
        self.authenticate_management_token(request, current)?;
        let existing = self.load_key_ring()?;
        let mut ring = match &existing {
            Some(ring) => ring.clone(),
            None => self.initial_key_ring()?,
        };
        // Never silently discard or hide authentication owners from an older multi-key setup.
        if ring.keys.len() > 2 {
            return Err(GatewayError::conflict(
                "Existing key ring has more than two keys",
            ));
        }
        if slot >= ring.keys.len() {
            let token = replacement.as_ref().ok_or_else(|| {
                GatewayError::bad_request("Enter a key before enabling this slot")
            })?;
            ring.keys.push(StoredKey {
                id: uuid::Uuid::new_v4().to_string(),
                name: "管理密钥二".into(),
                token_hash: hash_token(token)?,
                created_at: OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
                sealed_token: None,
                enabled: false,
            });
        }
        if let Some(token) = replacement.as_ref() {
            for (index, key) in ring.keys.iter().enumerate() {
                if index != slot
                    && Argon2::default()
                        .verify_password(
                            token.as_bytes(),
                            &PasswordHash::new(&key.token_hash).map_err(|_| invalid_record())?,
                        )
                        .is_ok()
                {
                    return Err(GatewayError::conflict("Management key already exists")
                        .with_code("console_key_duplicate"));
                }
            }
            ring.keys[slot].token_hash = hash_token(token)?;
            ring.keys[slot].sealed_token =
                Some(self.seal_management_key(&guard, &ring.keys[slot], token)?);
        } else if ring.keys[slot].sealed_token.is_none()
            && ring
                .verify(self, current)
                .is_ok_and(|id| id == ring.keys[slot].id)
        {
            // Preserve the legacy key for display before it becomes disabled.
            ring.keys[slot].sealed_token =
                Some(self.seal_management_key(&guard, &ring.keys[slot], current)?);
        }
        ring.keys[slot].enabled = enabled;
        ring.revision = uuid::Uuid::new_v4().to_string();
        self.save_management_key_ring(&guard, &ring, existing.is_none())?;
        self.secret_grants.lock().clear();
        Ok(())
    }
}
