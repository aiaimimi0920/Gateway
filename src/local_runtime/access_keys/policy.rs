//! Local direct-route authorization; restrictions are checked again before selection.
use super::*;
use crate::auth::{adapter::AuthRequest, session::AuthenticatedSession};
use crate::routing::candidate::RouteCandidate;
use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Restrictions {
    scope: Option<Vec<String>>,
    scopes: Option<Vec<String>>,
    models: Option<Vec<String>>,
    provider_ids: Option<Vec<String>>,
}

fn restrictions(key: Option<&serde_json::Value>) -> Result<Restrictions, GatewayError> {
    let parsed: Restrictions = match key {
        None | Some(serde_json::Value::Null) => Restrictions::default(),
        Some(value) => serde_json::from_value(value.clone()).map_err(|_| {
            GatewayError::bad_request(
                "Local key metadata supports scope/scopes, models and providerIds string arrays",
            )
        })?,
    };
    if parsed.scope.is_some() && parsed.scopes.is_some() {
        return Err(GatewayError::bad_request("Use either scope or scopes"));
    }
    for values in [
        &parsed.scope,
        &parsed.scopes,
        &parsed.models,
        &parsed.provider_ids,
    ]
    .into_iter()
    .flatten()
    {
        if values.len() > 128
            || values
                .iter()
                .any(|value| value.trim().is_empty() || value.len() > 256)
        {
            return Err(GatewayError::bad_request(
                "Invalid local key restriction list",
            ));
        }
    }
    Ok(parsed)
}

pub(super) fn validate_input(input: &UpsertAccessKeyInput) -> Result<(), GatewayError> {
    for value in [
        &input.owner_id,
        &input.resolved_project_id,
        &input.resolved_tenant_id,
        &input.display_name,
    ] {
        if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
            return Err(GatewayError::bad_request(
                "Key identity and display name must contain 1-256 characters",
            ));
        }
    }
    if !matches!(
        input.owner_type.trim(),
        "user" | "tenant" | "project" | "platform"
    ) {
        return Err(GatewayError::bad_request("Invalid access key owner type"));
    }
    if !matches!(input.key_kind.trim(), "user" | "normal") || !input.bundle_ids.is_empty() {
        return Err(GatewayError::bad_request("Local keys use normal route access; server bundles and aggregate keys require server storage"));
    }
    let prefix = input.public_key_prefix.trim();
    if prefix.is_empty()
        || prefix.len() > 32
        || !prefix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(GatewayError::bad_request("Invalid public key prefix"));
    }
    if let Some(value) = &input.expires_at {
        time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
            .map_err(|_| GatewayError::bad_request("expiresAt must be an RFC3339 timestamp"))?;
    }
    restrictions(input.metadata.as_ref())?;
    Ok(())
}

pub(in crate::local_runtime) fn ensure_active(
    key: &GatewayAccessKeyView,
) -> Result<(), GatewayError> {
    if key.status != "active" || key.revoked_at.is_some() {
        return Err(GatewayError::unauthorized("Access key is revoked"));
    }
    if let Some(value) = &key.expires_at {
        let expiry =
            time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
                .map_err(|_| GatewayError::unauthorized("Invalid access key expiry"))?;
        if expiry <= time::OffsetDateTime::now_utc() {
            return Err(GatewayError::unauthorized("Access key has expired"));
        }
    }
    Ok(())
}

impl LocalRuntime {
    pub async fn local_access_models(
        &self,
        id: &str,
        routes: &crate::routing::config::RouteConfigStore,
    ) -> Result<Vec<crate::routing::config::ModelInfo>, GatewayError> {
        let mut tx = self.pool.begin().await.map_err(storage_error)?;
        let key = read(&mut tx, id).await?;
        ensure_active(&key)?;
        let policy = restrictions(key.metadata.as_ref())?;
        tx.commit().await.map_err(storage_error)?;
        let snapshot = routes.snapshot();
        let mut models = snapshot.list_models();
        models.retain(|model| {
            policy
                .models
                .as_ref()
                .is_none_or(|allowed| allowed.contains(&model.id))
                && policy.provider_ids.as_ref().is_none_or(|allowed| {
                    snapshot
                        .resolve_candidates(Some(&model.id))
                        .iter()
                        .any(|candidate| allowed.contains(&candidate.provider_account_id))
                })
        });
        Ok(models)
    }

    pub async fn authenticate_access_key(
        &self,
        request: &AuthRequest,
    ) -> Result<Option<AuthenticatedSession>, GatewayError> {
        let Some(token) = request.credential().filter(|value| value.len() <= 512) else {
            return Ok(None);
        };
        let payload: Option<String> =
            sqlx::query_scalar("SELECT payload FROM local_access_keys WHERE token_hash = ?")
                .bind(digest(token))
                .fetch_optional(&self.pool)
                .await
                .map_err(storage_error)?;
        let Some(payload) = payload else {
            return Ok(None);
        };
        let key = decode(&payload)?;
        ensure_active(&key)?;
        let scopes = crate::db::scope_list_from_metadata(key.metadata.as_ref());
        let endpoint = request.path.trim_start_matches("/v1/").replace('/', ".");
        if !scopes
            .iter()
            .any(|scope| scope == "relay" || scope == &endpoint)
        {
            return Err(denied("Access key does not allow this endpoint"));
        }
        // A JSON field update cannot overwrite a concurrent revocation/rotation.
        sqlx::query("UPDATE local_access_keys SET payload = json_set(payload, '$.lastUsedAt', ?) WHERE id = ?")
            .bind(now()).bind(&key.id).execute(&self.pool).await.map_err(storage_error)?;
        Ok(Some(AuthenticatedSession {
            project_id: key.resolved_project_id,
            tenant_id: key.resolved_tenant_id,
            user_id: None,
            credential_ref: None,
            scopes,
            api_key_id: None,
            user_credential_id: None,
            access_key_id: Some(key.id),
            access_key_kind: Some("normal".into()),
        }))
    }

    pub async fn authorize_local_candidates(
        &self,
        id: &str,
        model: Option<&str>,
        candidates: &mut Vec<RouteCandidate>,
    ) -> Result<(), GatewayError> {
        self.authorize_local_candidates_with_fallback_authorization(id, model, candidates)
            .await
            .map(|_| ())
    }

    /// Return explicit IDs from the same validated key transaction, never from auto candidates.
    pub async fn authorize_local_candidates_with_fallback_authorization(
        &self,
        id: &str,
        model: Option<&str>,
        candidates: &mut Vec<RouteCandidate>,
    ) -> Result<Option<Vec<String>>, GatewayError> {
        let mut tx = self.pool.begin().await.map_err(storage_error)?;
        let key = read(&mut tx, id).await?;
        ensure_active(&key)?;
        let policy = restrictions(key.metadata.as_ref())?;
        if let Some(models) = policy.models {
            if !model.is_some_and(|model| models.iter().any(|allowed| allowed == model)) {
                return Err(denied("Access key does not allow this model"));
            }
        }
        if let Some(providers) = policy.provider_ids.as_ref() {
            candidates.retain(|candidate| providers.contains(&candidate.provider_account_id));
            if candidates.is_empty() {
                return Err(denied(
                    "Access key has no permitted provider for this request",
                ));
            }
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(policy.provider_ids)
    }
}

pub fn local_key_id<'a>(
    state: &crate::state::AppState,
    session: Option<&'a AuthenticatedSession>,
) -> Option<&'a str> {
    state.local_runtime.as_ref()?;
    session?
        .access_key_id
        .as_deref()
        .filter(|id| id.starts_with(ID_PREFIX))
}
