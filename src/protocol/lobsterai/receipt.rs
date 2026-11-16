//! Preserve delivery before account admission; unknown lifetime stays unknown, never six hours.
use super::{
    atom, transport, AuthorizationContext, Client, LoginError, ReceivedTokens, SessionMaterial,
};
use serde_json::Value;

/// Temporary protocol receipt. Management must durably retain it before publishing a rotated token.
pub struct ReceivedSession {
    client: Client,
    response: Option<ReceivedTokens>,
    previous: Option<SessionTokens>,
}
pub struct SessionTokens {
    client: Client,
    material: SessionMaterial,
}
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Identity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    yid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
}
impl ReceivedSession {
    pub(super) fn capture(
        client: Client,
        response: Value,
        previous: Option<SessionTokens>,
    ) -> Result<Self, LoginError> {
        let response = ReceivedTokens::capture(response)?;
        Ok(Self {
            client,
            response: Some(response),
            previous,
        })
    }
    /// This does not save or merge accounts; receipt ownership remains with the caller on rejection.
    pub fn admit(&mut self) -> Result<SessionTokens, LoginError> {
        let receipt = self.response.as_ref().ok_or(LoginError::ReceiptConsumed)?;
        let material = if let Some(previous) = &self.previous {
            receipt.interpret_successor(
                &previous.material.context,
                previous.access_token(),
                previous.refresh_token(),
            )?
        } else {
            receipt.admit_initial(self.client.installation_id)?
        };
        let tokens = SessionTokens {
            client: self.client.clone(),
            material,
        };
        self.response = None;
        self.previous = None;
        Ok(tokens)
    }
}
impl ReceivedSession {
    pub fn into_receipt(mut self) -> Result<ReceivedTokens, LoginError> {
        self.response.take().ok_or(LoginError::ReceiptConsumed)
    }
}
impl SessionTokens {
    pub fn restore(client: Client, material: SessionMaterial) -> Result<Self, LoginError> {
        material
            .context
            .validate_binding(&material.access_token, &material.refresh_token)?;
        if client.installation_id != material.context.installation_id() {
            return Err(LoginError::IdentityMismatch);
        }
        Ok(Self { client, material })
    }
    pub fn into_material(self) -> SessionMaterial {
        self.material
    }
    pub fn context(&self) -> &AuthorizationContext {
        &self.material.context
    }

    pub fn access_token(&self) -> &str {
        &self.material.access_token
    }
    pub fn refresh_token(&self) -> &str {
        &self.material.refresh_token
    }
    /// Endpoint-declared identity, not a verified JWT subject or management principal.
    pub fn user_id(&self) -> &str {
        self.material.context.user_id()
    }
    pub async fn model_catalog(&self) -> Result<Vec<super::DeclaredModel>, LoginError> {
        super::catalog::parse(
            transport::catalog(
                &self.client.http,
                &self.material.access_token,
                &self.client,
                self.user_id(),
            )
            .await?,
        )
    }
    /// Only a 401 for this exact access token authorizes a refresh attempt. The durable owner
    /// must reserve the old generation before execution; a 403 is never treated as token expiry.
    pub fn into_refresh_grant(
        self,
        rejected_access: &str,
        status: u16,
    ) -> Result<RefreshGrant, LoginError> {
        if status != 401 {
            return Err(LoginError::Rejected);
        }
        if rejected_access != self.material.access_token {
            return Err(LoginError::IdentityMismatch);
        }
        Ok(RefreshGrant { previous: self })
    }
}
pub struct RefreshGrant {
    previous: SessionTokens,
}
impl RefreshGrant {
    pub async fn execute(self) -> Result<ReceivedSession, LoginError> {
        let client = self.previous.client.clone();
        let mut body = client.attribution();
        body["refreshToken"] = self.previous.material.refresh_token.clone().into();
        body["userId"] = self.previous.user_id().into();
        let response =
            transport::post(&client.http, "/api/auth/refresh", &body, transport::BUDGET).await?;
        ReceivedSession::capture(client, response, Some(self.previous))
    }
}
fn secret(value: &Value) -> Result<&str, LoginError> {
    value
        .as_str()
        .filter(|s| atom(s, 32_768))
        .ok_or(LoginError::InvalidResponse)
}
impl Identity {
    pub(super) fn user_id(&self) -> &str {
        self.yid
            .as_deref()
            .or(self.user_id.as_deref())
            .expect("admitted identity")
    }
    pub(super) fn validate(&self) -> Result<(), LoginError> {
        Self::parse(&serde_json::to_value(self).map_err(|_| LoginError::InvalidResponse)?)
            .map(|_| ())
    }

    fn parse(value: &Value) -> Result<Self, LoginError> {
        if !value.is_object() {
            return Err(LoginError::InvalidResponse);
        }
        let field = |name: &str| -> Result<Option<String>, LoginError> {
            value
                .get(name)
                .map(|value| {
                    value
                        .as_str()
                        .filter(|s| {
                            !s.is_empty() && s.len() <= 512 && !s.chars().any(char::is_control)
                        })
                        .map(str::to_owned)
                        .ok_or(LoginError::InvalidResponse)
                })
                .transpose()
        };
        let identity = Self {
            yid: field("yid")?,
            user_id: field("userId")?,
        };
        if identity.yid.is_none() && identity.user_id.is_none() {
            return Err(LoginError::InvalidResponse);
        }
        Ok(identity)
    }
}
/// Scan only the official account-context locations, including conflicting nested declarations.
fn personal_context(value: &Value) -> Result<bool, LoginError> {
    let Some(object) = value.as_object() else {
        return Err(LoginError::InvalidResponse);
    };
    let mut personal = false;
    if let Some(mode) = object.get("accountMode") {
        match mode.as_str() {
            Some("personal") => personal = true,
            Some("enterprise") => return Err(LoginError::EnterpriseUnsupported),
            _ => return Err(LoginError::AccountContextRequired),
        }
    }
    if object.get("enterpriseId").is_some_and(|v| !v.is_null())
        || value
            .pointer("/quota/subscriptionStatus")
            .and_then(Value::as_str)
            == Some("enterprise")
    {
        return Err(LoginError::EnterpriseUnsupported);
    }
    for key in [
        "enterpriseContext",
        "accountContext",
        "organizationContext",
        "context",
        "user",
        "data",
    ] {
        if let Some(nested) = object.get(key).filter(|v| !v.is_null()) {
            personal |= personal_context(nested)?;
        }
    }
    Ok(personal)
}
impl std::fmt::Debug for ReceivedSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LobsterReceivedSession { material: <redacted>, admitted: false }")
    }
}
impl std::fmt::Debug for SessionTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LobsterSessionTokens { material: <redacted>, expiry: unknown }")
    }
}
impl std::fmt::Debug for RefreshGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LobsterRefreshGrant { material: <redacted> }")
    }
}

pub(super) fn interpret(
    response: &Value,
    installation: uuid::Uuid,
    previous: Option<(&AuthorizationContext, &str)>,
) -> Result<SessionMaterial, LoginError> {
    if installation.is_nil() {
        return Err(LoginError::InvalidInput);
    }
    let data = &response["data"];
    let explicit_personal = personal_context(data)?;
    if previous.is_none() && !explicit_personal {
        return Err(LoginError::AccountContextRequired);
    }
    let identity = match data.get("user") {
        Some(user) => Identity::parse(user)?,
        None => previous
            .map(|(context, _)| context.identity().clone())
            .ok_or(LoginError::InvalidResponse)?,
    };
    if previous.is_some_and(|(context, _)| context.identity() != &identity) {
        return Err(LoginError::IdentityMismatch);
    }
    let access = secret(&data["accessToken"])?;
    let refresh = match data.get("refreshToken") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.trim().is_empty() => None,
        Some(value) => Some(secret(value)?),
    }
    .or_else(|| previous.map(|(_, refresh)| refresh))
    .ok_or(LoginError::InvalidResponse)?;
    let generation = previous
        .map(|(context, _)| context.successor_generation())
        .transpose()?
        .unwrap_or(0);
    Ok(SessionMaterial {
        access_token: access.into(),
        refresh_token: refresh.into(),
        context: AuthorizationContext::admitted(
            installation,
            identity,
            generation,
            access,
            refresh,
            previous.and_then(|(context, _)| context.reauthorization_id()),
        ),
    })
}
