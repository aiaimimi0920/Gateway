//! Explicit per-request Preview opt-in; never enable it by a guessed model or provider default.
use super::{Contract, GatewayError};
use crate::protocol::{canonical::CanonicalRelayRequest, tool_choice::pack_gemini_tool_choice};
use serde_json::{json, Value};

pub(super) fn apply(
    contract: Contract,
    req: &CanonicalRelayRequest,
    body: &mut Value,
) -> Result<(), GatewayError> {
    let invalid = || invalid(contract);
    let invariant = || invariant(contract);
    // Responses intentionally does not copy arbitrary unknown fields into extra. raw_body is the
    // existing lossless owner: read only this option here, without expanding the shared adapter.
    let Some(config) = req
        .raw_body
        .get("toolConfig")
        .or_else(|| req.extra.get("toolConfig"))
    else {
        return Ok(());
    };
    let Some(option) = config
        .get("functionCallingConfig")
        .and_then(|function| function.get("streamFunctionCallArguments"))
    else {
        return Ok(());
    };
    let enabled = option.as_bool().ok_or_else(invalid)?;
    let config = body
        .as_object_mut()
        .ok_or_else(invariant)?
        .entry("toolConfig")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(invalid)?;
    let function = config
        .entry("functionCallingConfig")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(invalid)?;
    // Chat/Messages extra may have replaced the whole toolConfig during shared packing. Restore
    // canonical public tool_choice (including allowed names); the Preview switch is not a choice.
    if let Some(Value::Object(choice)) = pack_gemini_tool_choice(req.tool_choice.as_ref()) {
        function.remove("mode");
        function.remove("allowedFunctionNames");
        function.extend(choice);
    }
    function.insert("streamFunctionCallArguments".into(), json!(enabled));
    Ok(())
}

fn invalid(contract: Contract) -> GatewayError {
    GatewayError::bad_request(format!("{} streamFunctionCallArguments must be a JSON boolean in toolConfig.functionCallingConfig.", contract.label))
        .with_code(format!("{}_invalid_tool_stream_option", contract.code_prefix))
}
fn invariant(contract: Contract) -> GatewayError {
    GatewayError::server_error(format!(
        "{} prepared body is not an object.",
        contract.label
    ))
    .with_code(format!("{}_plan_invariant", contract.code_prefix))
}

#[cfg(test)]
#[path = "../vertex_express/options/tests.rs"]
mod tests;
