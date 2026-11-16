//! Live account-local model IDs, not guessed prices, tool support, entitlement or health evidence.
use super::{atom, AuthorizationContext, OAuthError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub id: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelCatalog {
    pub models: Vec<Model>,
}
impl ModelCatalog {
    pub fn parse(value: Value) -> Result<Self, OAuthError> {
        if value.get("error").is_some() {
            return Err(OAuthError::InvalidResponse);
        }
        let data = value
            .get("data")
            .and_then(Value::as_array)
            .filter(|v| v.len() <= 2048)
            .ok_or(OAuthError::InvalidResponse)?;
        let mut seen = std::collections::HashSet::new();
        let mut models = Vec::with_capacity(data.len());
        for entry in data {
            let id = entry
                .get("id")
                .and_then(Value::as_str)
                .filter(|s| atom(s, 512))
                .ok_or(OAuthError::InvalidResponse)?;
            if !seen.insert(id) {
                return Err(OAuthError::InvalidResponse);
            }
            models.push(Model { id: id.into() });
        }
        Ok(Self { models })
    }
}
impl AuthorizationContext {
    pub async fn model_catalog(
        &self,
        http: &rquest::Client,
        access: &str,
    ) -> Result<ModelCatalog, OAuthError> {
        self.validate_access(access)?;
        let value = super::transport::get_models(
            http,
            &format!("{}/models", self.inference_target().base_url()),
            access,
        )
        .await?;
        ModelCatalog::parse(value)
    }
}
