//! Native route commits preserve unrelated providers and never expose OAuth tokens to the UI.
use super::material;
use crate::{error::GatewayError, protocol::chatgpt::codex_client, state::AppState};

pub(super) fn validate_provider(state: &AppState, id: &str) -> Result<(), GatewayError> {
    if !state
        .route_config
        .snapshot()
        .document()
        .providers
        .iter()
        .any(|p| p.id == id && p.preset.as_deref() == Some("chatgpt-codex-oauth-official-api"))
    {
        return Err(GatewayError::bad_request(
            "Save a ChatGPT OAuth pool before starting login",
        ));
    }
    if state.route_config_runtime.is_none() {
        return Err(GatewayError::bad_request(
            "Route configuration is read-only",
        ));
    }
    Ok(())
}

pub(super) async fn save(
    state: &AppState,
    provider: &str,
    group: &str,
    material: &material::Material,
    deadline: std::time::Instant,
) -> Result<String, GatewayError> {
    validate_provider(state, provider)?;
    let models = codex_client::models(
        state.upstream_client.client(),
        &material.token,
        &material.account,
    )
    .await?;
    if std::time::Instant::now() >= deadline {
        return Err(GatewayError::bad_request(
            "ChatGPT login expired before import",
        ));
    }
    let mut credential = material::credential(material, group, &models)?;
    let snapshot = state.route_config.snapshot();
    let mut document = snapshot.document().clone();
    let target = document
        .providers
        .iter_mut()
        .find(|p| {
            p.id == provider && p.preset.as_deref() == Some("chatgpt-codex-oauth-official-api")
        })
        .ok_or_else(|| GatewayError::bad_request("ChatGPT pool was removed during login"))?;
    for id in material::GROUPS {
        if !target
            .credential_identity_categories
            .iter()
            .any(|category| category["id"].as_str() == Some(id))
        {
            target
                .credential_identity_categories
                .push(serde_json::json!({
                    "id": id, "label": id.replace('-', " "), "pool_target_size": 30,
                    "auto_refill_enabled": false, "auto_prune_enabled": false
                }));
        }
    }
    if let Some(existing) = target
        .credentials
        .iter_mut()
        .find(|c| c.headers.get("Chatgpt-Account-Id") == Some(&material.account))
    {
        existing.api_key = credential.api_key.clone();
        existing.refresh_token = credential.refresh_token.clone();
        existing.refresh_endpoint = credential.refresh_endpoint.clone();
        existing.refresh_client_id = credential.refresh_client_id.clone();
        existing.token_expires_in_secs = credential.token_expires_in_secs;
        existing.expires_at = credential.expires_at.clone();
        existing.account_name = credential.account_name.clone();
        existing.credential_identity_category_id =
            credential.credential_identity_category_id.clone();
        existing.supported_models = credential.supported_models.clone();
        credential = existing.clone();
    } else {
        target.credentials.push(credential.clone());
    }
    for model in models {
        if !target.supported_models.contains(&model) {
            target.supported_models.push(model);
        }
    }
    if target
        .default_model
        .as_ref()
        .is_none_or(|model| !target.supported_models.contains(model))
    {
        target.default_model = codex_client::preferred_model(&credential.supported_models);
    }
    target
        .headers
        .insert("User-Agent".into(), codex_client::USER_AGENT.into());
    state
        .route_config_runtime
        .as_ref()
        .ok_or_else(|| GatewayError::bad_request("Route configuration is read-only"))?
        .commit_document(
            snapshot.revision().id(),
            document,
            Some("Import ChatGPT OAuth credential".into()),
        )
        .await
        .map_err(|_| {
            GatewayError::bad_request(
                "Could not save ChatGPT credential; refresh configuration and retry import",
            )
        })?;
    Ok(credential.id.unwrap_or_default())
}
