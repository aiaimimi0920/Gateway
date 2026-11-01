//! Additive native route commits; never writes a browser state file or a second credential store.
use super::material::{self, Material};
use crate::{
    error::GatewayError,
    routing::config::{ModelRoute, RouteConfigYaml},
    state::AppState,
};
use std::time::Instant;

pub(super) fn validate_provider(state: &AppState, provider: &str) -> Result<(), GatewayError> {
    if state.route_config_runtime.is_none() {
        return Err(GatewayError::bad_request(
            "Route configuration is read-only",
        ));
    }
    if !state
        .route_config
        .snapshot()
        .document()
        .providers
        .iter()
        .any(|p| p.id == provider && p.preset.as_deref() == Some("antigravity-oauth"))
    {
        return Err(GatewayError::bad_request(
            "Save an Antigravity OAuth provider before login",
        ));
    }
    Ok(())
}

pub(super) async fn save(
    state: &AppState,
    provider: &str,
    material: &Material,
    deadline: Instant,
) -> Result<String, GatewayError> {
    validate_provider(state, provider)?;
    let (project, models) = crate::upstream::antigravity::account_catalog(
        state.upstream_client.client(),
        &material.token,
    )
    .await?;
    if Instant::now() >= deadline {
        return Err(GatewayError::bad_request(
            "Antigravity login expired before import",
        ));
    }
    let snapshot = state.route_config.snapshot();
    let mut document = snapshot.document().clone();
    let id = update(&mut document, provider, material, &project, &models)?;
    state
        .route_config_runtime
        .as_ref()
        .ok_or_else(|| GatewayError::bad_request("Route configuration is read-only"))?
        .commit_document(
            snapshot.revision().id(),
            document,
            Some("Import Antigravity OAuth credential".into()),
        )
        .await
        .map_err(|e| {
            GatewayError::bad_request(
                "Could not save Antigravity credential; refresh configuration and retry import",
            )
            .with_code(e.code())
        })?;
    Ok(id)
}

fn update(
    document: &mut RouteConfigYaml,
    provider: &str,
    material: &Material,
    project: &str,
    models: &[String],
) -> Result<String, GatewayError> {
    let mut credential = material::credential(material, project, models)?;
    let target = document
        .providers
        .iter_mut()
        .find(|p| p.id == provider && p.preset.as_deref() == Some("antigravity-oauth"))
        .ok_or_else(|| {
            GatewayError::bad_request("Antigravity provider was removed during login")
        })?;
    if let Some(existing) = target.credentials.iter_mut().find(|c| {
        c.extra_body
            .get("antigravity_account_id")
            .and_then(|v| v.as_str())
            == Some(&material.account)
    }) {
        // Preserve operator policy, enabled state and ID on re-authorization.
        existing.api_key = credential.api_key.clone();
        existing.refresh_token = credential.refresh_token.clone();
        existing.refresh_endpoint = credential.refresh_endpoint.clone();
        existing.refresh_client_id = credential.refresh_client_id.clone();
        existing.token_expires_in_secs = credential.token_expires_in_secs;
        existing.expires_at = credential.expires_at.clone();
        existing.account_name = credential.account_name.clone();
        existing.supported_models = credential.supported_models.clone();
        existing.extra_body.extend(credential.extra_body.clone());
        credential = existing.clone();
    } else {
        target.credentials.push(credential.clone());
    }
    for model in models {
        if !target.supported_models.contains(model) {
            target.supported_models.push(model.clone());
        }
    }
    if target
        .default_model
        .as_ref()
        .is_none_or(|m| !target.supported_models.contains(m))
    {
        target.default_model = models.first().cloned();
    }
    for model in models {
        if let Some(route) = document
            .model_routes
            .iter_mut()
            .find(|r| r.pattern == *model)
        {
            if !route.provider_ids.iter().any(|id| id == provider) {
                route.provider_ids.push(provider.to_owned());
            }
        } else {
            document.model_routes.push(ModelRoute {
                pattern: model.clone(),
                provider_ids: vec![provider.to_owned()],
                priority: 10,
                enabled: true,
            });
        }
    }
    Ok(credential.id.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn antigravity_reauthorization_preserves_ids_disabled_policy_and_other_providers() {
        let mut doc:RouteConfigYaml=serde_json::from_value(json!({"providers":[
            {"id":"other","base_url":"https://example.invalid","api_key":"unchanged","supported_models":["catalog-model"]},
            {"id":"pool","preset":"antigravity-oauth","base_url":crate::protocol::antigravity::BASE_URL,"credentials":[
                {"id":"keep-id","api_key":"old","enabled":false,"extra_body":{"antigravity_account_id":"google-sub","other":true}}]}],
            "model_routes":[{"pattern":"catalog-model","provider_ids":["other"],"enabled":false,"priority":99}]})).unwrap();
        let material = Material {
            client_id: "synthetic-client".into(),
            token: "new".into(),
            refresh: "rotating".into(),
            account: "google-sub".into(),
            email: "a@example.invalid".into(),
            expires: 3600,
            expires_at: crate::db::format_timestamp(
                time::OffsetDateTime::now_utc() + time::Duration::hours(1),
            ),
        };
        assert_eq!(
            update(
                &mut doc,
                "pool",
                &material,
                "real-project",
                &["catalog-model".into()]
            )
            .unwrap(),
            "keep-id"
        );
        assert_eq!(
            update(
                &mut doc,
                "pool",
                &material,
                "real-project",
                &["catalog-model".into()]
            )
            .unwrap(),
            "keep-id"
        );
        let credential = &doc.providers[1].credentials[0];
        assert_eq!(doc.providers[1].credentials.len(), 1);
        assert_eq!(doc.providers[0].api_key, "unchanged");
        assert_eq!(credential.enabled, Some(false));
        assert_eq!(credential.extra_body["other"], true);
        assert_eq!(credential.refresh_token.as_deref(), Some("rotating"));
        assert_eq!(doc.model_routes[0].provider_ids, ["other", "pool"]);
        assert!(!doc.model_routes[0].enabled);
        assert_eq!(doc.model_routes[0].priority, 99);
        assert!(crate::routing::config::RouteConfigStore::from_document(doc).is_ok());
    }
}
