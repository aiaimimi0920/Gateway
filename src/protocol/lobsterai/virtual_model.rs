//! Request-owned selection from the catalog or virtual resolver; never replace the frozen tariff.
use super::{catalog, metadata, response_error, transport, validation, DeclaredModel};
use crate::{
    error::GatewayError, protocol::canonical::CanonicalRelayRequest,
    routing::candidate::ProviderAccountPayload, upstream::client::UpstreamClient,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
pub(crate) const CAPABILITY: &str = "virtual-models-v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Selection {
    pub preset: String,
    pub model: DeclaredModel,
}
impl Selection {
    pub(crate) fn trace(&self) -> serde_json::Value {
        if is_virtual(&self.preset) {
            json!({"preset":self.preset,"modelId":self.model.model_id,"apiFormat":self.model.api_format})
        } else {
            json!({"source":"catalog","modelId":self.model.model_id,"apiFormat":self.model.api_format})
        }
    }
}
/// Convert a copy for the admitted Chat tariff or public OpenAI output. Native Messages
/// usage remains authoritative for audit/observation; overflow stays unknown.
pub(crate) fn chat_usage(
    selection: Option<&Selection>,
    usage: Option<crate::protocol::canonical::TokenUsage>,
) -> Option<crate::protocol::canonical::TokenUsage> {
    let mut usage = usage?;
    if selection.is_some_and(|s| s.model.api_format.as_deref() == Some("anthropic")) {
        usage = super::usage::inclusive(usage)?;
    }
    Some(usage)
}
pub(crate) fn is_virtual(id: &str) -> bool {
    matches!(id, "yd-fast" | "yd-ultra")
}
pub(crate) fn observed_model(payload: &ProviderAccountPayload, fallback: &str) -> String {
    if super::owns_payload(payload) {
        if let Ok(metadata) = metadata::read(payload) {
            if let Some(selection) = metadata.selection {
                return selection.model.model_id;
            }
        }
    }
    fallback.to_owned()
}
pub(crate) async fn prepare(
    client: &UpstreamClient,
    payload: &mut ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    id: &str,
    request_id: &str,
) -> Result<Option<Selection>, GatewayError> {
    if !super::owns_payload(payload) {
        return Ok(None);
    }
    validation::validate(payload)?;
    let declared = validation::model(payload, id, req)?;
    if !is_virtual(id) {
        if declared.api_format.as_deref() != Some("anthropic") {
            return Ok(None);
        }
        // Record the native usage dialect for public projection, audit and frozen cash settlement.
        // A direct selection performs no resolve request and advertises no virtual capability.
        let selection = Selection {
            preset: id.into(),
            model: declared,
        };
        metadata::selected(payload, selection.clone());
        super::runtime::plan(payload, req, id, req.stream)?;
        return Ok(Some(selection));
    }
    // No caller conversation identifier is forwarded. Stateless public requests own this run UUID.
    let body = json!({"sessionId":request_id,"requiresImage":false});
    let response = tokio::time::timeout(client.timeout.min(transport::BUDGET), async {
        let response = client.lobsterai_http.post(transport::target(&format!("/api/models/{id}/resolve")))
            .json(&body).default_headers(false).redirect(rquest::redirect::Policy::none())
            .timeout(client.timeout.min(transport::BUDGET)).bearer_auth(&payload.api_key)
            .header("content-type", "application/json").header("accept", "application/json")
            .header("user-agent", concat!("NeuroGateway/", env!("CARGO_PKG_VERSION"), " lobsterai"))
            .header("X-LobsterAI-Client-Version", super::runtime::VERSION)
            .header("X-LobsterAI-Client-Capabilities", CAPABILITY)
            .send().await.map_err(|_| failed())?;
        super::runtime::admit_status(&response)?;
        if super::response::is_sse(&response)? { return Err(failed()); }
        let bytes = crate::protocol::upstream_body::collect_bounded_upstream_stream_with_limit_and_provider(
            response.bytes_stream(), "LobsterAI model resolution", transport::TOKEN_LIMIT, None)
            .await.map_err(|_| failed())?;
        crate::protocol::unique_json::parse(&bytes).map_err(|_| failed())
    }).await.map_err(|_| failed())??;
    if response["code"].as_i64() != Some(0) {
        return Err(failed());
    }
    let mut models = catalog::parse(json!({"code":0,"data":[response["data"].clone()]}))
        .map_err(|_| failed())?;
    let model = models.pop().ok_or_else(failed)?;
    validation::concrete(&model, req)?;
    let selection = Selection {
        preset: id.into(),
        model,
    };
    metadata::selected(payload, selection.clone());
    // Reject unsupported concrete request controls before recording an inference attempt.
    super::runtime::plan(payload, req, id, req.stream)?;
    Ok(Some(selection))
}
fn failed() -> GatewayError {
    response_error().with_code("lobsterai_model_resolution_failed")
}
