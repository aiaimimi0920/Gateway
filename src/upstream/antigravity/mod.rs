//! Independent Antigravity Cloud Code owner. Calls never create or attach a browser.
mod fingerprint;
mod native;
mod response;
#[cfg(test)]
mod tests;
pub(crate) mod transport;
mod wire;

use crate::{
    error::{FallbackHint, GatewayError},
    protocol::{
        antigravity,
        canonical::{CanonicalRelayRequest, CanonicalRelayResponse},
    },
    routing::candidate::ProviderAccountPayload,
};
use serde_json::{json, Value};
use std::time::Duration;

pub(crate) fn failure(code: &str, message: &str) -> GatewayError {
    let mut error = GatewayError::service_unavailable(message)
        .with_provider(antigravity::ADAPTER)
        .with_code(code);
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Antigravity generation must not be replayed after ambiguous delivery".into(),
    };
    error
}

pub async fn account_catalog(
    client: &rquest::Client,
    token: &str,
) -> Result<(String, Vec<String>), GatewayError> {
    tokio::time::timeout(Duration::from_secs(45), async {
        let account = transport::json(client, antigravity::BASE_URL, token, "loadCodeAssist", &json!({"metadata":{"ideType":"ANTIGRAVITY"}})).await?;
        let project = account["cloudaicompanionProject"].as_str()
            .or_else(|| account["cloudaicompanionProject"]["id"].as_str())
            .filter(|v| !v.trim().is_empty() && v.len() <= 256)
            .ok_or_else(|| failure("antigravity_project_missing", "Account has no activated Cloud Code project; activate Antigravity normally first"))?.to_owned();
        let catalog = transport::json(client, antigravity::BASE_URL, token, "fetchAvailableModels", &json!({"project":project})).await?;
        Ok((project, catalog_models(&catalog)?))
    }).await.map_err(|_| failure("antigravity_catalog_timeout", "Antigravity account discovery timed out"))?
}

fn catalog_models(value: &Value) -> Result<Vec<String>, GatewayError> {
    let object = value["models"]
        .as_object()
        .filter(|m| !m.is_empty() && m.len() <= 512)
        .ok_or_else(|| {
            failure(
                "antigravity_catalog_invalid",
                "Antigravity did not return a bounded model catalog",
            )
        })?;
    let mut models: Vec<_> = object
        .keys()
        .filter(|s| !s.is_empty() && s.len() <= 200 && !s.chars().any(char::is_control))
        .cloned()
        .collect();
    models.sort();
    if models.is_empty() {
        return Err(failure(
            "antigravity_catalog_empty",
            "Antigravity catalog has no usable model IDs",
        ));
    }
    Ok(models)
}

pub async fn probe(
    client: &rquest::Client,
    payload: &ProviderAccountPayload,
) -> Result<(), GatewayError> {
    transport::admit_base(&payload.base_url)?;
    if payload.api_key.trim().is_empty() {
        return Err(failure(
            "antigravity_credential_missing",
            "Import an Antigravity OAuth account first",
        ));
    }
    account_catalog(client, &payload.api_key).await.map(|_| ())
}

pub async fn execute(
    client: &rquest::Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    transport::admit_base(&payload.base_url)?;
    if payload.api_key.trim().is_empty() {
        return Err(failure(
            "antigravity_credential_missing",
            "Import an Antigravity OAuth account first",
        ));
    }
    let body = wire::envelope(payload, req, model)?;
    tokio::time::timeout(timeout.min(Duration::from_secs(180)), async {
        // Native hub always uses the SSE upstream; downstream streaming remains Canonical-owned.
        let method = "streamGenerateContent?alt=sse";
        let bytes = transport::post(
            client,
            &payload.base_url,
            &payload.api_key,
            method,
            &body,
            32 * 1024 * 1024,
        )
        .await?;
        let response = response::decode(&bytes, true)?;
        crate::protocol::gemini_api::parse_generate_content_response(&response, model).map_err(
            |_| {
                failure(
                    "antigravity_response_invalid",
                    "Antigravity did not return a valid generateContent response",
                )
            },
        )
    })
    .await
    .map_err(|_| {
        failure(
            "antigravity_deadline_exceeded",
            "Antigravity generation timed out; not replayed",
        )
    })?
}
