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
        if let Some(local) = &self.0.local_runtime {
            return local.access_catalog().await;
        }
        db::list_access_catalog(
            self.postgres()?,
            self.0.config.gateway_api_key_secret.as_deref(),
        )
        .await
    }

    pub async fn save(
        &self,
        id: Option<&str>,
        input: db::UpsertAccessKeyInput,
    ) -> Result<db::GatewayAccessKeyView, GatewayError> {
        if let Some(local) = &self.0.local_runtime {
            return local.save_access_key(id, input).await;
        }
        db::save_access_key(
            self.postgres()?,
            &self.0.redis_pool,
            id,
            self.0.config.gateway_api_key_secret.as_deref(),
            input,
        )
        .await
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

    pub async fn delete(&self, id: &str) -> Result<db::DeleteAccessKeyResult, GatewayError> {
        if let Some(local) = &self.0.local_runtime {
            return local.delete_access_key(id).await;
        }
        db::delete_access_key(self.postgres()?, &self.0.redis_pool, id).await
    }
}
