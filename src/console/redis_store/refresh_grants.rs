//! Namespace-scoped grant CAS binds the active authority and immutable revision bytes atomically.
use super::*;
use crate::console::refresh_grants::{
    invalid, valid_digest, GrantPhase, GrantReceipt, GrantRecord, GrantRequest, GrantReservation,
    GrantResult, MAX_GRANT_BYTES,
};

const RESERVE: &str = r#"
if redis.call('GET', KEYS[1]) ~= ARGV[1] or redis.call('GET', KEYS[2]) ~= ARGV[2] then return -1 end
if (redis.call('GET', KEYS[3]) or '') ~= ARGV[3] then return 0 end
if ARGV[3] ~= '' and redis.call('GET', KEYS[4]) ~= ARGV[4] then return 0 end
if redis.call('EXISTS', KEYS[5]) ~= 0 then return 0 end
redis.call('SET', KEYS[5], ARGV[5])
redis.call('SET', KEYS[3], ARGV[6])
return 1
"#;
const TRANSITION: &str = r#"
if redis.call('GET', KEYS[1]) ~= ARGV[1] then return 0 end
if redis.call('GET', KEYS[2]) ~= ARGV[2] then return 0 end
if ARGV[4] ~= '' and (redis.call('GET', KEYS[3]) ~= ARGV[4] or redis.call('GET', KEYS[4]) ~= ARGV[5]) then return 0 end
redis.call('SET', KEYS[1], ARGV[3])
return 1
"#;
const READ: &str = r#"
if redis.call('STRLEN', KEYS[1]) > tonumber(ARGV[1]) then return {'invalid'} end
local raw = redis.call('GET', KEYS[1])
if not raw then return {'absent'} end
return {'value', raw}
"#;

type Error = RouteConfigRedisStoreError;
pub(super) fn invalid_state() -> Error {
    Error::from_persistence(invalid())
}
pub(super) fn unavailable(_: impl std::fmt::Display) -> Error {
    Error::unavailable("OAuth refresh grant storage is unavailable")
}

impl RouteConfigRedisStore {
    pub fn refresh_grant_key(&self, family: &str, material: Option<&str>) -> Result<String, Error> {
        if !valid_digest(family) || material.is_some_and(|id| !valid_digest(id)) {
            return Err(invalid_state());
        }
        Ok(format!(
            "gw:console:{}:oauth-refresh:{family}:{}",
            self.keys().namespace(),
            material.unwrap_or("head")
        ))
    }

    pub(super) async fn read_grant_value(
        &self,
        pool: &Pool,
        key: &str,
        limit: usize,
    ) -> Result<Option<String>, Error> {
        let mut connection = pool.get().await.map_err(unavailable)?;
        let reply: Vec<String> = redis::Script::new(READ)
            .key(key)
            .arg(limit)
            .invoke_async(&mut connection)
            .await
            .map_err(unavailable)?;
        match reply.as_slice() {
            [status] if status == "absent" => Ok(None),
            [status, value] if status == "value" => Ok(Some(value.clone())),
            _ => Err(invalid_state()),
        }
    }

    pub(super) async fn read_grant(
        &self,
        pool: &Pool,
        family: &str,
        material: &str,
    ) -> Result<Option<(String, GrantRecord)>, Error> {
        let Some(raw) = self
            .read_grant_value(
                pool,
                &self.refresh_grant_key(family, Some(material))?,
                MAX_GRANT_BYTES,
            )
            .await?
        else {
            return Ok(None);
        };
        let record = GrantRecord::from_json(&raw).map_err(Error::from_persistence)?;
        if record.family_id() != family || record.material_id() != material {
            return Err(invalid_state());
        }
        Ok(Some((raw, record)))
    }

    pub(super) async fn grant_revision_raw(
        &self,
        pool: &Pool,
        expected: &RouteConfigRedisRevision,
    ) -> Result<String, Error> {
        let mut connection = pool.get().await.map_err(unavailable)?;
        let raw: String = connection
            .get(self.keys().revision_key(expected.metadata().id()))
            .await
            .map_err(unavailable)?;
        let actual = RouteConfigRedisRevision::from_json(&raw).map_err(|_| invalid_state())?;
        if !crate::console::refresh_grants::same_authority_document(&actual, expected)
            .map_err(|_| invalid_state())?
        {
            return Err(invalid_state());
        }
        Ok(raw)
    }

    pub async fn load_refresh_grant(
        &self,
        pool: &Pool,
        locator: &crate::console::refresh_grants::GrantLocator,
    ) -> Result<Option<GrantRecord>, Error> {
        locator.validate().map_err(Error::from_persistence)?;
        let record = self
            .read_grant(pool, &locator.family, &locator.material)
            .await?
            .map(|(_, record)| record);
        if record
            .as_ref()
            .is_some_and(|record| !locator.matches(record))
        {
            return Err(invalid_state());
        }
        Ok(record)
    }

    pub async fn reserve_refresh_grant(
        &self,
        pool: &Pool,
        revision: &RouteConfigRedisRevision,
        request: &GrantRequest,
    ) -> Result<GrantReservation, Error> {
        self.ensure_single_node(pool).await?;
        if !request.matches(revision) {
            return Ok(GrantReservation::Stale);
        }
        let family = request.family_id();
        let material = request.material_id();
        if let Some((_, record)) = self.read_grant(pool, &family, &material).await? {
            return Ok(GrantReservation::Existing(record));
        }
        let head_key = self.refresh_grant_key(&family, None)?;
        let head = self.read_grant_value(pool, &head_key, 64).await?;
        let (head_record_key, prior_raw) = if let Some(head) = &head {
            let (raw, prior) = self
                .read_grant(pool, &family, head)
                .await?
                .ok_or_else(invalid_state)?;
            if !request.can_follow(&prior) {
                return Ok(GrantReservation::Blocked);
            }
            (self.refresh_grant_key(&family, Some(head))?, raw)
        } else {
            (
                self.refresh_grant_key(&family, Some(&material))?,
                String::new(),
            )
        };
        let authority = self.grant_revision_raw(pool, revision).await?;
        let record = GrantRecord::sending(request);
        let raw = record.to_json().map_err(Error::from_persistence)?;
        let mut connection = pool.get().await.map_err(unavailable)?;
        let applied: i32 = redis::Script::new(RESERVE)
            .key(self.keys().active_revision_key())
            .key(self.keys().revision_key(revision.metadata().id()))
            .key(head_key)
            .key(head_record_key)
            .key(self.refresh_grant_key(&family, Some(&material))?)
            .arg(revision.metadata().id())
            .arg(authority)
            .arg(head.unwrap_or_default())
            .arg(prior_raw)
            .arg(raw)
            .arg(material)
            .invoke_async(&mut connection)
            .await
            .map_err(unavailable)?;
        match applied {
            1 => Ok(GrantReservation::Reserved(record)),
            -1 => Ok(GrantReservation::Stale),
            0 => Ok(GrantReservation::Contended),
            _ => Err(invalid_state()),
        }
    }

    pub async fn store_refresh_grant_receipt(
        &self,
        pool: &Pool,
        expected: &GrantRecord,
        receipt: &GrantReceipt,
    ) -> Result<GrantRecord, Error> {
        let desired = expected
            .with_receipt(receipt)
            .map_err(Error::from_persistence)?;
        self.transition_grant(pool, expected, &desired, None).await
    }

    pub async fn store_refresh_grant_result(
        &self,
        pool: &Pool,
        expected: &GrantRecord,
        result: &GrantResult,
    ) -> Result<GrantRecord, Error> {
        let desired = expected
            .with_result(result)
            .map_err(Error::from_persistence)?;
        self.transition_grant(pool, expected, &desired, None).await
    }

    pub async fn mark_refresh_grant_published(
        &self,
        pool: &Pool,
        revision: &RouteConfigRedisRevision,
        expected: &GrantRecord,
    ) -> Result<GrantRecord, Error> {
        let desired = expected
            .published(revision)
            .map_err(Error::from_persistence)?;
        self.transition_grant(pool, expected, &desired, Some(revision))
            .await
    }

    async fn transition_grant(
        &self,
        pool: &Pool,
        expected: &GrantRecord,
        desired: &GrantRecord,
        authority: Option<&RouteConfigRedisRevision>,
    ) -> Result<GrantRecord, Error> {
        self.ensure_single_node(pool).await?;
        expected.validate().map_err(Error::from_persistence)?;
        let desired_raw = desired.to_json().map_err(Error::from_persistence)?;
        let family = expected.family_id();
        let material = expected.material_id();
        let (raw, current) = self
            .read_grant(pool, &family, &material)
            .await?
            .ok_or_else(invalid_state)?;
        if current == *desired {
            return Ok(current);
        }
        if current.acknowledges_receipt(desired) {
            return Ok(current);
        }
        if current.phase() == GrantPhase::Published
            && current.same_attempt(expected)
            && current.result() == desired.result()
            && current.receipt() == desired.receipt()
        {
            return Ok(current);
        }
        if current != *expected {
            return Err(invalid_state());
        }
        let (revision_id, revision_raw, revision_key) = if let Some(revision) = authority {
            (
                revision.metadata().id(),
                self.grant_revision_raw(pool, revision).await?,
                self.keys().revision_key(revision.metadata().id()),
            )
        } else {
            ("", String::new(), self.keys().active_revision_key())
        };
        let mut connection = pool.get().await.map_err(unavailable)?;
        let applied: i32 = redis::Script::new(TRANSITION)
            .key(self.refresh_grant_key(&family, Some(&material))?)
            .key(self.refresh_grant_key(&family, None)?)
            .key(self.keys().active_revision_key())
            .key(revision_key)
            .arg(raw)
            .arg(material)
            .arg(desired_raw)
            .arg(revision_id)
            .arg(revision_raw)
            .invoke_async(&mut connection)
            .await
            .map_err(unavailable)?;
        if applied != 1 {
            return Err(invalid_state());
        }
        Ok(desired.clone())
    }
}
