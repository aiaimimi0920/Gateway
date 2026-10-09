//! Access-key operations select storage once, outside the HTTP contract.
use crate::{db, error::GatewayError, state::AppState};

pub struct AccessStore<'a>(pub &'a AppState);

impl AccessStore<'_> {
    fn postgres(&self) -> Result<&sqlx::PgPool, GatewayError> {
        self.0.pg_pool.as_ref().ok_or_else(|| {
            GatewayError::service_unavailable("Access key storage is not configured")
        })
    }

    pub async fn catalog(&self) -> Result<db::GatewayAccessCatalogView, GatewayError> {
        let mut catalog = if let Some(local) = &self.0.local_runtime {
            local.access_catalog().await?
        } else {
            db::list_access_catalog(
                self.postgres()?,
                self.0.config.gateway_api_key_secret.as_deref(),
            )
            .await?
        };
        crate::access_balance::AccessBalanceStore::from_state(self.0)?
            .attach_cash(&mut catalog.balances)
            .await?;
        Ok(catalog)
    }

    pub async fn save(
        &self,
        id: Option<&str>,
        input: db::UpsertAccessKeyInput,
    ) -> Result<db::GatewayAccessKeyView, GatewayError> {
        self.save_with_quota(id, input, None).await
    }

    pub async fn save_with_quota(
        &self,
        id: Option<&str>,
        input: db::UpsertAccessKeyInput,
        quota: Option<&crate::access_balance::quota::KeyQuotaInput>,
    ) -> Result<db::GatewayAccessKeyView, GatewayError> {
        crate::access_key_groups::validate_input(self.0, &input)?;
        if quota.is_some_and(|quota| quota.mode == "cash_prepaid")
            && !matches!(input.key_kind.as_str(), "normal" | "user")
        {
            return Err(GatewayError::bad_request(
                "Cash quotas require a normal access key",
            ));
        }
        if let Some(local) = &self.0.local_runtime {
            return local.save_access_key_with_quota(id, input, quota).await;
        }
        db::access::save_access_key_with_quota(
            self.postgres()?,
            &self.0.redis_pool,
            id,
            self.0.config.gateway_api_key_secret.as_deref(),
            input,
            quota,
        )
        .await
    }

    pub async fn secret(&self, id: &str) -> Result<String, GatewayError> {
        if let Some(local) = &self.0.local_runtime {
            return local.access_key_secret(id).await;
        }
        db::access::read_access_key_secret(
            self.postgres()?,
            id,
            self.0.config.gateway_api_key_secret.as_deref(),
        )
        .await
    }

    pub async fn metadata(&self, id: &str) -> Result<Option<serde_json::Value>, GatewayError> {
        if let Some(local) = &self.0.local_runtime {
            return local.access_key_metadata(id).await;
        }
        let key = db::find_access_key_auth_by_id(self.postgres()?, id)
            .await?
            .ok_or_else(|| GatewayError::unauthorized("Access key not found"))?;
        db::validate_access_key_auth(&key)?;
        Ok(key.metadata)
    }

    pub async fn rotate(&self, id: &str) -> Result<db::GatewayAccessKeyView, GatewayError> {
        if let Some(local) = &self.0.local_runtime {
            return local.rotate_access_key(id).await;
        }
        db::rotate_access_key(
            self.postgres()?,
            &self.0.redis_pool,
            id,
            self.0.config.gateway_api_key_secret.as_deref(),
        )
        .await
    }

    pub async fn revoke(&self, id: &str, reason: Option<&str>) -> Result<(), GatewayError> {
        if let Some(local) = &self.0.local_runtime {
            return local.revoke_access_key(id, reason).await;
        }
        db::revoke_access_key(self.postgres()?, &self.0.redis_pool, id, reason).await
    }

    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), GatewayError> {
        if let Some(local) = &self.0.local_runtime {
            return local.set_access_key_enabled(id, enabled).await;
        }
        db::access::set_access_key_enabled(self.postgres()?, &self.0.redis_pool, id, enabled).await
    }

    pub async fn delete(&self, id: &str) -> Result<db::DeleteAccessKeyResult, GatewayError> {
        if let Some(local) = &self.0.local_runtime {
            return local.delete_access_key(id).await;
        }
        db::delete_access_key(self.postgres()?, &self.0.redis_pool, id).await
    }
}
