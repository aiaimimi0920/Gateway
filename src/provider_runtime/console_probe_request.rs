//! Backward-compatible single-prompt or bounded test-plan input; never an implicit call.
use crate::error::GatewayError;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ConsoleProbeRequest {
    #[serde(default)]
    pub prompt: String,
    pub model: Option<String>,
    pub credential_ids: Option<Vec<String>>,
    pub test_plan: Option<crate::credential_test_policy::CredentialTestPolicy>,
    pub scope: Option<ConsoleTestScope>,
    pub plan_id: Option<String>,
}

pub(crate) use crate::credential_test_plan::TestScope as ConsoleTestScope;

impl ConsoleProbeRequest {
    pub fn parse(body: &[u8]) -> Result<Option<Self>, GatewayError> {
        if body.is_empty() {
            return Ok(None);
        }
        let invalid = || {
            GatewayError::bad_request("Invalid manual test: prompt 1..8192 bytes, exact model up to 256 bytes, and 1..128 unique credential IDs.")
            .with_code("console_probe_input_invalid")
        };
        if body.len() > 262_144 {
            return Err(invalid());
        }
        let request: Self = serde_json::from_slice(body).map_err(|_| invalid())?;
        if (request.test_plan.is_none()
            && request.plan_id.is_none()
            && request.prompt.trim().is_empty())
            || request.prompt.len() > 8192
            || request.model.as_ref().is_some_and(|model| {
                model.is_empty()
                    || model.len() > 256
                    || model.trim() != model
                    || model.contains(['*', '?'])
                    || model.chars().any(char::is_control)
            })
            || request.credential_ids.as_ref().is_some_and(|ids| {
                ids.is_empty()
                    || ids.len() > 128
                    || ids.iter().any(|id| id.is_empty() || id.len() > 256)
                    || ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len()
            })
        {
            return Err(invalid());
        }
        if request.plan_id.as_ref().is_some_and(|id| {
            !crate::credential_test_policy::exact_model(id)
                || request.test_plan.is_some()
                || request.scope.is_some()
                || !request.prompt.is_empty()
                || request.model.is_some()
        }) {
            return Err(invalid());
        }
        if let Some(plan) = &request.test_plan {
            plan.validate().map_err(|message| {
                GatewayError::bad_request(message).with_code("console_probe_input_invalid")
            })?;
            if !request.prompt.is_empty() || request.model.is_some() {
                return Err(invalid());
            }
        }
        if request.scope.as_ref().is_some_and(|scope| match scope {
            ConsoleTestScope::Pool => false,
            ConsoleTestScope::Subpool { id } | ConsoleTestScope::Account { id } => {
                !crate::credential_test_policy::exact_model(id)
            }
        }) || (request.scope.is_some() && request.test_plan.is_none())
        {
            return Err(invalid());
        }
        Ok(Some(request))
    }
}
