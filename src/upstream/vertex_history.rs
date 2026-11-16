//! v1 correlates parallel function results by name/order, not the public protocols' call IDs.
#[cfg(test)]
use super::vertex_product::{DIM, EXPRESS, OAUTH};
use super::{common::RequestPlan, vertex_product::Contract};
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalToolCall, MessageRole};
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use serde_json::{json, Value};
use std::collections::HashMap;
mod options;
mod signed;

pub(crate) fn prepare_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    mut plan: RequestPlan,
    model: &str,
) -> Result<RequestPlan, GatewayError> {
    // Generic/legacy Gemini stays on its previous path. Product identity is never inferred from ID.
    if let Some(contract) = super::vertex_product::for_payload(payload) {
        let invariant = || invariant(contract);
        signed::reject_override(contract, req)?;
        let contents = plan
            .body
            .as_mut()
            .and_then(|body| body.get_mut("contents"))
            .ok_or_else(invariant)?;
        *contents = normalize(contract, req, contents.take(), model)?;
        options::apply(contract, req, plan.body.as_mut().ok_or_else(invariant)?)?;
    }
    Ok(plan)
}

fn invalid(contract: Contract) -> GatewayError {
    GatewayError::bad_request(format!(
        "Invalid or incomplete {} function history.",
        contract.label
    ))
    .with_code(format!("{}_invalid_tool_history", contract.code_prefix))
}

fn invariant(contract: Contract) -> GatewayError {
    GatewayError::server_error(format!(
        "{} prepared history did not match canonical messages.",
        contract.label
    ))
    .with_code(format!("{}_history_invariant", contract.code_prefix))
}

struct Pending<'a> {
    contract: Contract,
    calls: Vec<&'a CanonicalToolCall>,
    index: HashMap<&'a str, usize>,
    results: Vec<Option<Value>>,
    remaining: usize,
    started: bool,
}
impl<'a> Pending<'a> {
    fn new(contract: Contract) -> Self {
        Self {
            contract,
            calls: Vec::new(),
            index: HashMap::new(),
            results: Vec::new(),
            remaining: 0,
            started: false,
        }
    }
    fn extend(&mut self, calls: &'a [CanonicalToolCall]) -> Result<(), GatewayError> {
        let contract = self.contract;
        let invalid = || invalid(contract);
        if self.started {
            return Err(invalid());
        }
        for call in calls {
            let id = call
                .id
                .as_deref()
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(invalid)?;
            let name = call.name.as_deref().filter(|v| !v.trim().is_empty());
            if name.is_none()
                || !matches!(call.call_type.as_str(), "function" | "function_call")
                || self.index.insert(id, self.calls.len()).is_some()
            {
                return Err(invalid());
            }
            // Validate canonical arguments before trusting shared preparation's permissive {} repair.
            if let Some(raw) = &call.arguments {
                if !serde_json::from_str::<Value>(raw)
                    .map_err(|_| invalid())?
                    .is_object()
                {
                    return Err(invalid());
                }
            }
            self.calls.push(call);
            self.results.push(None);
            self.remaining += 1;
        }
        Ok(())
    }
    fn accept(
        &mut self,
        id: Option<&str>,
        name: Option<&str>,
        mut result: Value,
    ) -> Result<(), GatewayError> {
        let contract = self.contract;
        let invalid = || invalid(contract);
        let invariant = || invariant(contract);
        let index = *self
            .index
            .get(id.ok_or_else(invalid)?)
            .ok_or_else(invalid)?;
        let expected_name = self.calls[index].name.as_deref().unwrap();
        if self.results[index].is_some()
            || name.is_some_and(|v| v != expected_name)
            || !result["response"].is_object()
        {
            return Err(invalid());
        }
        result["name"] = json!(expected_name);
        result.as_object_mut().ok_or_else(invariant)?.remove("id");
        self.results[index] = Some(result);
        self.remaining -= 1;
        self.started = true;
        Ok(())
    }
    fn finish(self) -> Value {
        let parts: Vec<_> = self.results.into_iter()
            .map(|result| json!({"functionResponse":result.expect("all pending results were accepted")})).collect();
        json!({"role":"user","parts":parts})
    }
}

fn normalize(
    contract: Contract,
    req: &CanonicalRelayRequest,
    contents: Value,
    model: &str,
) -> Result<Value, GatewayError> {
    let invalid = || invalid(contract);
    let invariant = || invariant(contract);
    let messages: Vec<_> = req
        .messages
        .iter()
        .filter(|msg| msg.role != MessageRole::System)
        .collect();
    let Value::Array(contents) = contents else {
        return Err(invariant());
    };
    if messages.len() != contents.len() {
        return Err(invariant());
    }
    let mut output: Vec<Value> = Vec::new();
    let mut pending: Option<Pending<'_>> = None;
    for (message, mut content) in messages.into_iter().zip(contents) {
        let Value::Array(mut parts) = content["parts"].take() else {
            return Err(invariant());
        };
        signed::reject_untyped(contract, &parts)?;
        if message.role == MessageRole::Tool {
            if parts.len() != 1 || parts[0].get("functionResponse").is_none() {
                return Err(invariant());
            }
            let result = parts[0]["functionResponse"].take();
            let group = pending.as_mut().ok_or_else(invalid)?;
            group.accept(
                message.tool_call_id.as_deref(),
                message.name.as_deref(),
                result,
            )?;
            if group.remaining == 0 {
                output.push(pending.take().unwrap().finish());
            }
            continue;
        }
        // Raw content may not inject untracked calls/results around canonical ID correlation.
        let call_count = parts
            .iter()
            .filter(|part| part.get("functionCall").is_some())
            .count();
        if call_count != message.tool_calls.len()
            || parts.iter().any(|part| {
                ["functionResponse", "function_call", "function_response"]
                    .iter()
                    .any(|k| part.get(k).is_some())
            })
            || (message.role != MessageRole::Assistant && call_count != 0)
        {
            return Err(invalid());
        }
        if message.role == MessageRole::Assistant {
            if !message.tool_calls.is_empty() {
                pending
                    .get_or_insert_with(|| Pending::new(contract))
                    .extend(&message.tool_calls)?;
            } else if pending.as_ref().is_some_and(|group| group.started) {
                return Err(invalid());
            }
            // Responses represents parallel call items as adjacent assistant messages. Merge their
            // parts into one model turn, then emit every corresponding result in one ordered turn.
            if pending.is_some() && output.last().is_some_and(|last| last["role"] == "model") {
                output.last_mut().unwrap()["parts"]
                    .as_array_mut()
                    .ok_or_else(invariant)?
                    .append(&mut parts);
                continue;
            }
        } else if pending.is_some() {
            return Err(invalid());
        }
        content["parts"] = json!(parts);
        output.push(content);
    }
    if pending.is_some() {
        return Err(invalid());
    }
    signed::apply(contract, req, &mut output, model)?;
    for content in &mut output {
        for part in content["parts"].as_array_mut().ok_or_else(invariant)? {
            if let Some(call) = part.get_mut("functionCall") {
                call.as_object_mut().ok_or_else(invariant)?.remove("id");
            }
        }
    }
    Ok(json!(output))
}

#[cfg(test)]
mod source_tests;
#[cfg(test)]
#[path = "vertex_express/history/tests.rs"]
mod tests;
