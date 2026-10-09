//! In-place edits preserve key identity and keep replacement authentication atomic.
use super::*;

impl ConsoleAuthRuntime {
    pub fn edit_management_key(
        &self,
        request: &ConsoleRequestContext,
        current: &str,
        id: &str,
        name: &str,
        replacement: Option<&str>,
    ) -> Result<(), GatewayError> {
        let name = name.trim();
        let replacement = replacement.map(normalize_token).transpose()?;
        if name.is_empty()
            || name.chars().count() > 80
            || replacement.as_ref().is_some_and(|token| {
                token.len() > 4096 || !token.bytes().all(|c| (33..=126).contains(&c))
            })
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
        let index = ring
            .keys
            .iter()
            .position(|key| key.id == id)
            .ok_or_else(|| GatewayError::not_found("Management key not found"))?;
        if let Some(token) = replacement.as_ref() {
            if ring.verify(self, token).is_ok_and(|matched| matched != id) {
                return Err(GatewayError::conflict("Management key already exists")
                    .with_code("console_key_duplicate"));
            }
            ring.keys[index].token_hash = hash_token(token)?;
            ring.keys[index].sealed_token =
                Some(self.seal_management_key(&guard, &ring.keys[index], token)?);
        }
        ring.keys[index].name = name.into();
        ring.revision = uuid::Uuid::new_v4().to_string();
        self.save_management_key_ring(&guard, &ring, existing.is_none())?;
        if replacement.is_some() {
            self.secret_grants.lock().clear();
        }
        Ok(())
    }

    pub fn reveal_management_key(
        &self,
        request: &ConsoleRequestContext,
        current: &str,
        id: &str,
    ) -> Result<Option<String>, GatewayError> {
        // Hold the shared writer boundary so an edit/revoke cannot interleave with disclosure.
        let _guard = self
            .persistence
            .try_writer_lock()
            .map_err(persistence_to_gateway_error)?;
        self.authenticate_management_token(request, current)?;
        let Some(ring) = self.load_key_ring()? else {
            return if id == "initial" {
                Ok(Some(current.trim().into()))
            } else {
                Err(GatewayError::not_found("Management key not found"))
            };
        };
        let key = ring
            .keys
            .iter()
            .find(|key| key.id == id)
            .ok_or_else(|| GatewayError::not_found("Management key not found"))?;
        if ring.verify(self, current.trim())? == id {
            return Ok(Some(current.trim().into()));
        }
        key.sealed_token
            .as_ref()
            .map(|sealed| self.open_management_key(key, sealed))
            .transpose()
    }
}
