//! Explicit unlimited-key reuse; reseal for this data root, never copy another root's ciphertext.
use super::*;

impl LocalRuntime {
    pub async fn import_unlimited_access_key(
        &self,
        input: UpsertAccessKeyInput,
        token: &str,
    ) -> Result<GatewayAccessKeyView, GatewayError> {
        if !(16..=512).contains(&token.len()) || !token.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(GatewayError::bad_request(
                "Invalid imported access key token",
            ));
        }
        if let Some(expiry) = &input.expires_at {
            let expiry =
                time::OffsetDateTime::parse(expiry, &time::format_description::well_known::Rfc3339)
                    .map_err(|_| GatewayError::bad_request("Invalid imported access key expiry"))?;
            if expiry <= time::OffsetDateTime::now_utc() {
                return Err(GatewayError::bad_request(
                    "Cannot import an expired access key",
                ));
            }
        }
        let mut key = self
            .save_access_key_internal(None, input, None, Some(token))
            .await?;
        key.token = None;
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{input, request, setup};
    use super::*;

    #[tokio::test]
    async fn import_preserves_secret_restrictions_and_rejects_duplicate_or_invalid_tokens() {
        let (root, db) = setup().await;
        let token = "sk-gw-fixture-existing-token";
        for invalid in [
            "short",
            "sk-gw-fixture bad-token",
            "sk-gw-fixture\r\ninjected",
        ] {
            assert!(db
                .import_unlimited_access_key(input(), invalid)
                .await
                .is_err());
        }
        let key = db
            .import_unlimited_access_key(input(), token)
            .await
            .unwrap();
        assert!(key.token.is_none());
        assert_eq!(key.metadata, input().metadata);
        assert_eq!(db.access_key_secret(&key.id).await.unwrap(), token);
        assert!(db
            .import_unlimited_access_key(input(), token)
            .await
            .is_err());
        let mut auth_key = key.clone();
        auth_key.token = Some(token.into());
        assert!(db
            .authenticate_access_key(&request(&auth_key))
            .await
            .unwrap()
            .is_some());
        let mut forbidden = request(&auth_key);
        forbidden.path = "/v1/responses".into();
        assert!(db.authenticate_access_key(&forbidden).await.is_err());
        assert_eq!(db.access_catalog().await.unwrap().access_keys.len(), 1);
        let mut expired = input();
        expired.expires_at = Some("2000-01-01T00:00:00Z".into());
        assert!(db
            .import_unlimited_access_key(expired, "sk-gw-fixture-other-secret")
            .await
            .is_err());
        let stored: String =
            sqlx::query_scalar("SELECT payload FROM local_access_keys WHERE id = ?")
                .bind(&key.id)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert!(!stored.contains(token));
        db.close().await;
        let db = LocalRuntime::open(&root).await.unwrap();
        assert_eq!(db.access_key_secret(&key.id).await.unwrap(), token);
        assert!(db
            .authenticate_access_key(&request(&auth_key))
            .await
            .unwrap()
            .is_some());
        db.close().await;
        crate::local_runtime::test_support::remove_test_root(&root).await;
    }

    #[tokio::test]
    async fn failed_import_seal_rolls_back_without_changing_existing_key_or_balance() {
        let (root, db) = setup().await;
        let key = db.save_access_key(None, input()).await.unwrap();
        db.close().await;
        std::fs::remove_file(root.join("access-key-seal.json")).unwrap();
        let db = LocalRuntime::open(&root).await.unwrap();
        assert!(db
            .import_unlimited_access_key(input(), "sk-gw-fixture-reused-secret")
            .await
            .is_err());
        assert_eq!(db.access_catalog().await.unwrap().access_keys.len(), 1);
        assert!(db
            .authenticate_access_key(&request(&key))
            .await
            .unwrap()
            .is_some());
        let balances: i64 = sqlx::query_scalar("SELECT count(*) FROM local_balance_accounts")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(balances, 1);
        db.close().await;
        crate::local_runtime::test_support::remove_test_root(&root).await;
    }
}
