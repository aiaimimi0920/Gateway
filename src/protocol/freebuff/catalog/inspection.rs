//! Display-only interpretation: a foreign active instance is never acquired as an execution capability.
use super::{
    meter::{instant, MeterSnapshot},
    BusinessError, BusinessReply,
};
use serde::Serialize;
use serde_json::Value;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum InspectedSession {
    None,
    #[serde(rename_all = "camelCase")]
    Active {
        model_key: String,
        admitted_at: String,
        expires_at: String,
        remaining_ms: f64,
    },
    #[serde(rename_all = "camelCase")]
    Ended {
        in_grace: bool,
    },
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInspection {
    observed_at: String,
    pub(super) session: InspectedSession,
    pub(super) meter: MeterSnapshot,
}
fn text<'a>(body: &'a Value, field: &str) -> Result<&'a str, BusinessError> {
    body.get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 512 && !s.chars().any(char::is_control))
        .ok_or(BusinessError::InvalidResponse)
}
impl SessionInspection {
    /// HTTP status and full state are checked together. No raw reply or instance identity is exported.
    pub fn parse(
        reply: BusinessReply,
        selected_key: &str,
        now: OffsetDateTime,
    ) -> Result<Self, BusinessError> {
        Self::parse_ref(&reply, selected_key, now)
    }
    pub(super) fn parse_ref(
        reply: &BusinessReply,
        selected_key: &str,
        now: OffsetDateTime,
    ) -> Result<Self, BusinessError> {
        if reply.status != 200 || !reply.body.is_object() {
            return Err(BusinessError::InvalidResponse);
        }
        let body = &reply.body;
        let session = match text(body, "status")? {
            "none" => InspectedSession::None,
            "active" => {
                text(body, "instanceId")?;
                text(body, "accessTier")?;
                let model = text(body, "model")?;
                if !crate::credential_test_policy::exact_model(model) || model.starts_with("fbm1.")
                {
                    return Err(BusinessError::InvalidResponse);
                }
                let admitted = text(body, "admittedAt")?;
                let expires = text(body, "expiresAt")?;
                if instant(expires)? < instant(admitted)? {
                    return Err(BusinessError::InvalidResponse);
                }
                let remaining = body
                    .get("remainingMs")
                    .and_then(Value::as_f64)
                    .filter(|v| v.is_finite() && *v >= 0.0)
                    .ok_or(BusinessError::InvalidResponse)?;
                InspectedSession::Active {
                    model_key: model.into(),
                    admitted_at: admitted.into(),
                    expires_at: expires.into(),
                    remaining_ms: remaining,
                }
            }
            "ended" => {
                let in_grace = body.get("instanceId").is_some();
                if in_grace {
                    text(body, "instanceId")?;
                }
                InspectedSession::Ended { in_grace }
            }
            _ => return Err(BusinessError::InvalidResponse),
        };
        Ok(Self {
            observed_at: now
                .format(&Rfc3339)
                .map_err(|_| BusinessError::InvalidResponse)?,
            session,
            meter: MeterSnapshot::parse(body, selected_key, now)?,
        })
    }
}
