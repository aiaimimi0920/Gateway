//! Token-family identity ignores policy; an exact published expiry distinguishes safe renewal generations.
use super::invalid;
use crate::console::PersistenceError;
use crate::protocol::grok_oauth::AuthorizationContext;
use crate::routing::config::ProviderCredentialYaml;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GrantMaterial {
    pub(crate) endpoint: String,
    pub(crate) client: String,
    pub(crate) access: String,
    pub(crate) refresh: String,
    pub(crate) context: Option<AuthorizationContext>,
    pub(crate) expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) accio_sgcookie: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) kiro_oauth: Option<crate::protocol::kiro_oauth::AuthorizationContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) vertex_oauth: Option<crate::protocol::vertex_oauth::AuthorizationContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) dim_oauth: Option<crate::protocol::dim_oauth::AuthorizationContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) nous_portal: Option<crate::protocol::nous_portal::AuthorizationContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) lobsterai: Option<crate::protocol::lobsterai::AuthorizationContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) luma_oauth: Option<crate::protocol::luma_oauth::AuthorizationContext>,
}

impl GrantMaterial {
    pub(crate) fn from_credential(
        c: &ProviderCredentialYaml,
        provider_access: &str,
    ) -> Result<Self, PersistenceError> {
        let derived_endpoint = c
            .kiro_oauth
            .as_ref()
            .map(|context| context.token_endpoint());
        let (endpoint, client) = if let Some(context) = &c.luma_oauth {
            (
                crate::protocol::luma_oauth::TOKEN_ENDPOINT,
                context.client_id(),
            )
        } else if c.lobsterai.is_some() {
            (
                crate::protocol::lobsterai::TOKEN_ENDPOINT,
                crate::protocol::lobsterai::CLIENT_ATTRIBUTION,
            )
        } else if let Some(context) = &c.nous_portal {
            (
                crate::protocol::nous_portal::TOKEN_ENDPOINT,
                context.client_id(),
            )
        } else if let Some(context) = &c.dim_oauth {
            (context.token_endpoint(), context.client_id())
        } else if let Some(context) = &c.vertex_oauth {
            (
                crate::protocol::vertex_oauth::TOKEN_ENDPOINT,
                context.client_id(),
            )
        } else if let Some(context) = &c.grok_oauth {
            (
                crate::protocol::grok_oauth::TOKEN_ENDPOINT,
                context.client_id(),
            )
        } else if let Some(context) = &c.kiro_oauth {
            (
                derived_endpoint.as_deref().ok_or_else(invalid)?,
                context.client_id(),
            )
        } else {
            (
                c.refresh_endpoint.as_deref().ok_or_else(invalid)?,
                c.refresh_client_id.as_deref().unwrap_or_default(),
            )
        };
        let material = Self {
            endpoint: url::Url::parse(endpoint)
                .map_err(|_| invalid())?
                .to_string(),
            client: client.into(),
            access: c.api_key.as_deref().unwrap_or(provider_access).into(),
            refresh: c.refresh_token.clone().ok_or_else(invalid)?,
            context: c.grok_oauth.clone(),
            expires_at: if let Some(context) = &c.luma_oauth {
                super::luma::expiry(context)
            } else {
                crate::routing::config::refresh_absolute_expiry(c)
            },
            accio_sgcookie: c.accio_sgcookie.clone(),
            kiro_oauth: c.kiro_oauth.clone(),
            vertex_oauth: c.vertex_oauth.clone(),
            dim_oauth: c.dim_oauth.clone(),
            nous_portal: c.nous_portal.clone(),
            lobsterai: c.lobsterai.clone(),
            luma_oauth: c.luma_oauth.clone(),
        };
        material.validate()?;
        Ok(material)
    }

    pub(crate) fn validate(&self) -> Result<(), PersistenceError> {
        if self.luma_oauth.is_some() || self.endpoint == crate::protocol::luma_oauth::TOKEN_ENDPOINT
        {
            return super::luma::validate(self);
        }
        if let Some(context) = &self.lobsterai {
            if self.endpoint != crate::protocol::lobsterai::TOKEN_ENDPOINT
                || self.client != crate::protocol::lobsterai::CLIENT_ATTRIBUTION
                || self.expires_at.is_some()
                || self.context.is_some()
                || self.kiro_oauth.is_some()
                || self.vertex_oauth.is_some()
                || self.dim_oauth.is_some()
                || self.nous_portal.is_some()
                || self.accio_sgcookie.is_some()
                || context
                    .validate_binding(&self.access, &self.refresh)
                    .is_err()
            {
                return Err(invalid());
            }
        }

        let url = url::Url::parse(&self.endpoint).map_err(|_| invalid())?;
        if url.as_str() != self.endpoint
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || !(url.scheme() == "https"
                || (url.scheme() == "http"
                    && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))))
            || self.refresh.trim().is_empty()
            || [&self.access, &self.refresh, &self.client, &self.endpoint]
                .iter()
                .any(|s| s.len() > 65_536)
        {
            return Err(invalid());
        }
        if self.expires_at.as_ref().is_some_and(|value| {
            value.len() > 64
                || time::OffsetDateTime::parse(
                    value,
                    &time::format_description::well_known::Rfc3339,
                )
                .is_err()
        }) {
            return Err(invalid());
        }
        if let Some(context) = &self.context {
            if context.validate().is_err()
                || self.endpoint != crate::protocol::grok_oauth::TOKEN_ENDPOINT
                || self.client != context.client_id()
            {
                return Err(invalid());
            }
        }
        if let Some(cookie) = &self.accio_sgcookie {
            if self.endpoint != crate::protocol::accio_oauth::REFRESH_ENDPOINT
                || self.context.is_some()
                || !crate::protocol::accio_oauth::valid_cookie(cookie)
            {
                return Err(invalid());
            }
        }
        if let Some(context) = &self.kiro_oauth {
            if context.validate().is_err()
                || self.context.is_some()
                || self.accio_sgcookie.is_some()
                || self.endpoint != context.token_endpoint()
                || self.client != context.client_id()
                || !self.access.is_empty()
                    && !crate::protocol::kiro_oauth::business::valid_token(&self.access)
                || !crate::protocol::kiro_oauth::business::valid_token(&self.refresh)
            {
                return Err(invalid());
            }
        }
        if let Some(context) = &self.vertex_oauth {
            if context.validate().is_err()
                || self.context.is_some()
                || self.kiro_oauth.is_some()
                || self.accio_sgcookie.is_some()
                || self.endpoint != crate::protocol::vertex_oauth::TOKEN_ENDPOINT
                || self.client != context.client_id()
                || !self.access.is_empty()
                    && !crate::protocol::vertex_oauth::business::valid_token(&self.access)
                || !crate::protocol::vertex_oauth::business::valid_token(&self.refresh)
            {
                return Err(invalid());
            }
        }
        if let Some(context) = &self.dim_oauth {
            if context.validate().is_err()
                || self.context.is_some()
                || self.kiro_oauth.is_some()
                || self.vertex_oauth.is_some()
                || self.accio_sgcookie.is_some()
                || self.endpoint != context.token_endpoint()
                || self.client != context.client_id()
                || !crate::protocol::dim_oauth::business::valid_token(&self.access)
                || !crate::protocol::dim_oauth::business::valid_token(&self.refresh)
            {
                return Err(invalid());
            }
        }
        if let Some(context) = &self.nous_portal {
            use crate::protocol::nous_portal::{OAuthClient, TOKEN_ENDPOINT};
            if self.endpoint != TOKEN_ENDPOINT
                || self.client != context.client_id()
                || self.context.is_some()
                || self.kiro_oauth.is_some()
                || self.vertex_oauth.is_some()
                || self.dim_oauth.is_some()
                || self.accio_sgcookie.is_some()
                || !crate::protocol::nous_portal::atom(&self.refresh, 32_768)
                || !self
                    .expires_at
                    .as_deref()
                    .is_some_and(|expiry| context.matches_expiry(expiry))
                || context
                    .validate_binding(
                        &self.access,
                        &OAuthClient::new(&self.client).map_err(|_| invalid())?,
                    )
                    .is_err()
            {
                return Err(invalid());
            }
        }
        Ok(())
    }

    pub(crate) fn permits(&self, next: &Self) -> bool {
        if self.luma_oauth.is_some() || next.luma_oauth.is_some() {
            return self.validate().is_ok()
                && next.validate().is_ok()
                && self
                    .luma_oauth
                    .as_ref()
                    .zip(next.luma_oauth.as_ref())
                    .is_some_and(|(old, new)| old.permits_rotation(new));
        }
        next.validate().is_ok()
            && self.endpoint == next.endpoint
            && self.client == next.client
            && self.kiro_oauth == next.kiro_oauth
            && match (&self.lobsterai, &next.lobsterai) {
                (None, None) => true,
                (Some(old), Some(new)) => old.permits_rotation(new),
                _ => false,
            }
            && match (&self.nous_portal, &next.nous_portal) {
                (None, None) => true,
                (Some(old), Some(new)) => old.permits_rotation(new),
                _ => false,
            }
            && match (&self.dim_oauth, &next.dim_oauth) {
                (None, None) => true,
                (Some(old), Some(new)) => old.permits_rotation(new),
                _ => false,
            }
            && match (&self.vertex_oauth, &next.vertex_oauth) {
                (None, None) => true,
                (Some(old), Some(new)) => old.permits_rotation(new),
                _ => false,
            }
            && (self.accio_sgcookie.is_none() || next.accio_sgcookie.is_some())
            && match (&self.context, &next.context) {
                (None, None) => true,
                (Some(old), Some(new)) => old.permits_rotation(new, &next.access),
                _ => false,
            }
    }

    /// RFC3339 spelling is not a new authorization expiry (.120Z and .12Z are identical).
    pub(crate) fn equivalent(&self, other: &Self) -> bool {
        self.endpoint == other.endpoint
            && self.client == other.client
            && self.access == other.access
            && self.refresh == other.refresh
            && self.context == other.context
            && self.kiro_oauth == other.kiro_oauth
            && self.vertex_oauth == other.vertex_oauth
            && self.dim_oauth == other.dim_oauth
            && self.nous_portal == other.nous_portal
            && self.lobsterai == other.lobsterai
            && self.luma_oauth == other.luma_oauth
            && self.accio_sgcookie == other.accio_sgcookie
            && self.matches_expiry(other.expires_at.as_deref())
    }

    pub(crate) fn matches_expiry(&self, expiry: Option<&str>) -> bool {
        match (self.expires_at.as_deref(), expiry) {
            (None, None) => true,
            (Some(left), Some(right)) => {
                let parse = |value: &str| {
                    time::OffsetDateTime::parse(
                        value,
                        &time::format_description::well_known::Rfc3339,
                    )
                    .ok()
                };
                parse(left)
                    .zip(parse(right))
                    .is_some_and(|(left, right)| left == right)
            }
            _ => false,
        }
    }

    pub(crate) fn family_id(&self) -> String {
        let mut value = serde_json::json!([
            "gateway-refresh-grant-v1",
            self.endpoint,
            self.client,
            self.refresh
        ]);
        // Explicit browser reauthorization starts a new family even if refresh tokens repeat.
        // Legacy and other-provider identities remain byte-for-byte unchanged.
        if let Some(id) = self.lobsterai.as_ref().and_then(|c| c.reauthorization_id()) {
            value
                .as_array_mut()
                .expect("family identity is an array")
                .push(serde_json::json!({"lobsteraiReauthorization":id}));
        }
        digest(&value)
    }

    pub(crate) fn material_id(&self) -> String {
        let mut value = serde_json::json!([
            "gateway-refresh-material-v1",
            self.endpoint,
            self.client,
            self.refresh,
            self.access,
            self.context,
            self.expires_at
        ]);
        // An absent extension must preserve every existing material ID and family-head fence.
        if let Some(cookie) = &self.accio_sgcookie {
            value
                .as_array_mut()
                .expect("material identity is an array")
                .push(serde_json::json!({"accioSgcookie":cookie}));
        }
        if let Some(context) = &self.kiro_oauth {
            value
                .as_array_mut()
                .expect("material identity is an array")
                .push(serde_json::json!({"kiroOauth":context}));
        }
        if let Some(context) = &self.vertex_oauth {
            value
                .as_array_mut()
                .expect("material identity is an array")
                .push(serde_json::json!({"vertexOauth":context}));
        }
        if let Some(context) = &self.dim_oauth {
            value
                .as_array_mut()
                .expect("material identity is an array")
                .push(serde_json::json!({"dimOauth":context}));
        }
        if let Some(context) = &self.nous_portal {
            value
                .as_array_mut()
                .expect("material identity is an array")
                .push(serde_json::json!({"nousPortal":context}));
        }
        if let Some(context) = &self.lobsterai {
            value
                .as_array_mut()
                .expect("material identity is an array")
                .push(serde_json::json!({"lobsterai":context}));
        }
        if let Some(context) = &self.luma_oauth {
            value
                .as_array_mut()
                .expect("material identity is an array")
                .push(serde_json::json!({"lumaOauth":context}));
        }
        digest(&value)
    }
}

fn digest(value: &serde_json::Value) -> String {
    let mut value = value.clone();
    value.sort_all_objects();
    hex::encode(Sha256::digest(value.to_string().as_bytes()))
}
