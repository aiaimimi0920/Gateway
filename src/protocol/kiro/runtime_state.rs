//! Shared CLI conversation shape; authorization and destination remain with each product owner.
use crate::routing::candidate::ProviderAccountPayload;
use serde_json::Value;

pub(crate) fn valid(state: &Value, payload: &ProviderAccountPayload) -> bool {
    let Some(state) = state.as_object() else {
        return false;
    };
    let Some(current) = state
        .get("currentMessage")
        .and_then(|v| v.get("userInputMessage"))
    else {
        return false;
    };
    !state.keys().any(|k| {
        !matches!(
            k.as_str(),
            "conversationId" | "chatTriggerType" | "agentTaskType" | "currentMessage" | "history"
        )
    }) && state.get("chatTriggerType").and_then(Value::as_str) == Some("MANUAL")
        && state.get("agentTaskType").and_then(Value::as_str) == Some("vibe")
        && current["origin"] == "KIRO_CLI"
        && current["modelId"]
            .as_str()
            .is_some_and(|m| crate::provider_model_id::valid_for_payload(m, payload))
        && state
            .get("history")
            .and_then(Value::as_array)
            .is_some_and(|history| {
                history.iter().all(|entry| {
                    entry
                        .get("userInputMessage")
                        .is_none_or(|user| user["origin"] == "KIRO_CLI")
                })
            })
}
