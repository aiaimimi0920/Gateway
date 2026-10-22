//! Compact request planning: same governed route/credential, distinct operation.
use super::*;
use crate::upstream::common::RequestPlan;

#[cfg(test)]
#[path = "native_compact_tests.rs"]
mod tests;

pub(super) fn unsupported() -> GatewayError {
    GatewayError::bad_request("Compact requires a native Codex Responses route")
}

pub(super) fn plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<RequestPlan, GatewayError> {
    if !UpstreamClient::uses_native_codex(payload, req, model) || req.stream {
        return Err(unsupported());
    }
    // Reuse endpoint resolution, but never send generation-only profile defaults
    // (stream/store/tools/include) to the compact API.
    let mut plan = UpstreamClient::build_request_plan(payload, req, model, false)?;
    let mut url = url::Url::parse(&plan.url)
        .map_err(|_| GatewayError::bad_request("Invalid compact upstream URL"))?;
    url.set_path(&format!("{}/compact", url.path().trim_end_matches('/')));
    plan.url = url.into();
    let mut body = req.raw_body.clone();
    let map = body
        .as_object_mut()
        .ok_or_else(|| GatewayError::bad_request("Compact body must be an object"))?;
    map.insert("model".into(), Value::String(model.to_owned()));
    // Provider defaults are shared with create; only compact-valid defaults apply.
    // Unknown caller fields are retained for forward protocol compatibility.
    if let Some(extra) = &payload.extra_body {
        for key in [
            "instructions",
            "prompt_cache_key",
            "prompt_cache_options",
            "prompt_cache_retention",
            "service_tier",
        ] {
            if let Some(value) = extra.get(key) {
                map.entry(key.to_owned()).or_insert_with(|| value.clone());
            }
        }
    }
    plan.body = Some(body);
    Ok(plan)
}
