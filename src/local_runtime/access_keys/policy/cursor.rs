//! Cursor's private-account grant excludes implicit provider access and aliases.
use super::*;
use serde_json::Value;

impl LocalRuntime {
    pub(crate) async fn authorize_cursor_remote_account(
        &self,
        key_id: &str,
        presented_token: &str,
        public_model: &str,
        exact_provider_account: &str,
        required_scope: &str,
    ) -> Result<Value, GatewayError> {
        if ![
            "responses",
            "chat.completions",
            "gemini.generateContent",
            "cohere.chat",
            "bedrock.converse",
            "dashscope.text",
            "dashscope.multimodal",
        ]
        .contains(&required_scope)
        {
            return Err(rejected());
        }
        self.authorize_cursor_scope(
            key_id,
            presented_token,
            public_model,
            exact_provider_account,
            required_scope,
        )
        .await
    }

    async fn authorize_cursor_scope(
        &self,
        key_id: &str,
        presented_token: &str,
        public_model: &str,
        exact_provider_account: &str,
        required_scope: &str,
    ) -> Result<Value, GatewayError> {
        if !key_id.starts_with(ID_PREFIX)
            || presented_token.is_empty()
            || presented_token.len() > 512
        {
            return Err(rejected());
        }
        let payload: String = sqlx::query_scalar(
            "SELECT payload FROM local_access_keys WHERE id = ? AND token_hash = ?",
        )
        .bind(key_id)
        .bind(digest(presented_token))
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?
        .ok_or_else(rejected)?;
        let key = decode(&payload)?;
        ensure_active(&key)?;
        let metadata = key.metadata.ok_or_else(rejected)?;
        let policy = restrictions(Some(&metadata))?;
        let scopes = crate::db::scope_list_from_metadata(Some(&metadata));
        if !policy
            .provider_ids
            .is_some_and(|ids| ids.iter().any(|id| id == exact_provider_account))
            || !policy
                .models
                .is_some_and(|models| models.iter().any(|model| model == public_model))
            || !scopes
                .iter()
                .any(|scope| scope == required_scope || scope == "relay")
        {
            return Err(rejected());
        }
        // The dispatch owner must intersect account groups from this same value
        // and verify that the provider maps to one fixed, non-pooled credential.
        Ok(metadata)
    }
}

fn rejected() -> GatewayError {
    denied("Local key does not explicitly authorize this Cursor account and model")
}

#[cfg(test)]
mod tests {
    use crate::local_runtime::access_keys::tests::{input, setup};
    use serde_json::json;

    #[tokio::test]
    async fn cursor_remote_auth_requires_literal_account_model_scope_and_current_token() {
        let (root, db) = setup().await;
        let mut grant = input();
        grant.metadata = Some(json!({
            "scopes": ["chat.completions"],
            "providerIds": ["cursor-owner"],
            "models": ["cursor-personal"],
        }));
        let key = db.save_access_key(None, grant.clone()).await.unwrap();
        let token = key.token.as_deref().unwrap();
        for (account, model, allowed) in [
            ("cursor-owner", "cursor-personal", true),
            ("cursor", "cursor-personal", false),
            ("Cursor-Owner", "cursor-personal", false),
            ("cursor-owner", "different-model", false),
        ] {
            assert_eq!(
                db.authorize_cursor_remote_account(
                    &key.id,
                    token,
                    model,
                    account,
                    "chat.completions"
                )
                .await
                .is_ok(),
                allowed
            );
        }
        let rotated = db.rotate_access_key(&key.id).await.unwrap();
        assert!(db
            .authorize_cursor_remote_account(
                &key.id,
                token,
                "cursor-personal",
                "cursor-owner",
                "chat.completions"
            )
            .await
            .is_err());
        let new_token = rotated.token.as_deref().unwrap();
        assert!(db
            .authorize_cursor_remote_account(
                &key.id,
                new_token,
                "cursor-personal",
                "cursor-owner",
                "chat.completions"
            )
            .await
            .is_err());
        assert!(db
            .authorize_cursor_remote_account(
                &rotated.id,
                new_token,
                "cursor-personal",
                "cursor-owner",
                "chat.completions"
            )
            .await
            .is_ok());

        for metadata in [
            json!({"scopes":["chat.completions"],"models":["cursor-personal"]}),
            json!({"scopes":["chat.completions"],"providerIds":["cursor-owner"]}),
            json!({"scopes":["responses"],"providerIds":["cursor-owner"],"models":["cursor-personal"]}),
        ] {
            grant.metadata = Some(metadata);
            let ungranted = db.save_access_key(None, grant.clone()).await.unwrap();
            assert!(db
                .authorize_cursor_remote_account(
                    &ungranted.id,
                    ungranted.token.as_deref().unwrap(),
                    "cursor-personal",
                    "cursor-owner",
                    "chat.completions"
                )
                .await
                .is_err());
        }
        db.close().await;
        crate::local_runtime::test_support::remove_test_root(&root).await;
    }

    #[tokio::test]
    async fn cursor_remote_scope_rejects_mismatched_endpoint_and_rotated_key() {
        let (root, db) = setup().await;
        let mut grant = input();
        grant.metadata = Some(
            json!({"scopes":["responses"],"providerIds":["cursor-owner"],
            "models":["cursor-personal"]}),
        );
        let key = db.save_access_key(None, grant).await.unwrap();
        let token = key.token.as_deref().unwrap();
        assert!(db
            .authorize_cursor_remote_account(
                &key.id,
                token,
                "cursor-personal",
                "cursor-owner",
                "responses"
            )
            .await
            .is_ok());
        for scope in ["chat.completions", "messages"] {
            assert!(db
                .authorize_cursor_remote_account(
                    &key.id,
                    token,
                    "cursor-personal",
                    "cursor-owner",
                    scope,
                )
                .await
                .is_err());
        }
        db.rotate_access_key(&key.id).await.unwrap();
        assert!(db
            .authorize_cursor_remote_account(
                &key.id,
                token,
                "cursor-personal",
                "cursor-owner",
                "responses"
            )
            .await
            .is_err());
        db.close().await;
        crate::local_runtime::test_support::remove_test_root(&root).await;
    }
}
