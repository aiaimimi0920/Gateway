//! Credential-local declared wire capabilities, separate from successful-call evidence.
use super::{business, AuthorizationContext, DeclaredModel, Site, WireProtocol};
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCatalog {
    site: Site,
    client_id: String,
    subject: String,
    models: Vec<DeclaredModel>,
}
impl ModelCatalog {
    pub fn new(
        context: &AuthorizationContext,
        models: Vec<DeclaredModel>,
    ) -> Result<Self, GatewayError> {
        let catalog = Self {
            site: context.site(),
            client_id: context.client_id().into(),
            subject: context.subject().into(),
            models,
        };
        catalog.validate(context)?;
        Ok(catalog)
    }
    pub fn models(&self) -> &[DeclaredModel] {
        &self.models
    }
    pub fn validate(&self, context: &AuthorizationContext) -> Result<(), GatewayError> {
        context.validate().map_err(|_| business::invalid())?;
        if self.site != context.site()
            || self.client_id != context.client_id()
            || self.subject != context.subject()
            || self.models.len() > 2048
            || serde_json::to_vec(self).map_or(true, |v| v.len() > 2 * 1024 * 1024)
        {
            return Err(business::invalid());
        }
        let mut ids = std::collections::HashSet::new();
        for model in &self.models {
            let text = |s: &str, max| {
                !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control)
            };
            if !text(&model.id, 256)
                || !text(&model.name, 1024)
                || !ids.insert(&model.id)
                || [
                    &model.reasoning_efforts,
                    &model.input_modalities,
                    &model.output_modalities,
                    &model.service_tiers,
                ]
                .iter()
                .any(|values| values.len() > 32 || values.iter().any(|s| !text(s, 128)))
            {
                return Err(business::invalid());
            }
        }
        Ok(())
    }
    pub(crate) fn project(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
    ) -> Result<(ProviderAccountPayload, &'static str), GatewayError> {
        if payload.base_url.trim_end_matches('/') != business::base(self.site) {
            return Err(business::invalid());
        }
        let declaration = self.models.iter().find(|m| m.id == model).ok_or_else(|| {
            GatewayError::bad_request("DIM model has no saved wire declaration")
                .with_code("dim_model_not_declared")
        })?;
        let mut effective = payload.clone();
        effective.responses_path = None;
        effective.chat_completions_path = None;
        effective.messages_path = None;
        let family = match declaration.protocol {
            WireProtocol::Chat => {
                effective.chat_completions_path = Some("/chat/completions".into());
                "openai_chat"
            }
            WireProtocol::Responses => {
                effective.responses_path = Some("/responses".into());
                "openai_responses"
            }
            WireProtocol::Messages => {
                effective.adapter = "anthropic_compatible".into();
                effective.messages_path = Some("/messages".into());
                "anthropic_messages"
            }
            WireProtocol::Gemini => {
                effective.adapter = "gemini_api_compatible".into();
                effective.base_url = format!("{}/v1beta", self.site.issuer());
                "gemini_generate_content"
            }
        };
        super::runtime::validate(&effective, effective.canonical_adapter())?;
        Ok((effective, family))
    }
}
