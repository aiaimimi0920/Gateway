//! Internal Luma task metadata, not proof of the current upstream wire or asset contents.
use super::browser_worker::no_replay;
use crate::{
    error::GatewayError,
    upstream::lumalabs_runtime_helpers::PreparedLumalabsBrowserExecutorServiceInput,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TaskResult {
    pub(super) signed_url: String,
    pub(super) output_id: String,
    pub(super) action_id: Option<String>,
    pub(super) resolved_action_type: String,
    pub(super) realm_id: String,
    pub(super) media_operation: String,
    pub(super) artifact_field: String,
    pub(super) state: String,
}
fn failure() -> GatewayError {
    no_replay(
        GatewayError::server_error(
            "Luma Web executor did not return a complete matching task result; details omitted.",
        )
        .with_code("lumalabs_task_result_invalid"),
    )
}
impl TaskResult {
    pub(super) fn from_value(value: Value) -> Result<Self, GatewayError> {
        Self::check(serde_json::from_value(value).map_err(|_| failure())?)
    }
    pub(super) fn from_stdout(value: &str) -> Result<Self, GatewayError> {
        Self::check(serde_json::from_str(value).map_err(|_| failure())?)
    }
    fn check(result: Self) -> Result<Self, GatewayError> {
        // Old URL-only executors must fail closed, never silently replay a paid action.
        let exact = |value: &str| !value.is_empty() && value == value.trim();
        if result.state != "completed"
            || !["image", "video", "audio"].contains(&result.media_operation.as_str())
            || [
                &result.signed_url,
                &result.output_id,
                &result.resolved_action_type,
                &result.realm_id,
                &result.artifact_field,
            ]
            .iter()
            .any(|value| !exact(value))
            || result
                .action_id
                .as_deref()
                .is_some_and(|value| !exact(value))
        {
            return Err(failure());
        }
        Ok(result)
    }
    pub(super) fn validate(
        &self,
        expected: &PreparedLumalabsBrowserExecutorServiceInput,
    ) -> Result<(), GatewayError> {
        // The browser binds outputId to the one POST response. At the executor boundary,
        // retain and compare the context we do know instead of discarding it into a URL.
        if self.realm_id != expected.realm_id
            || Some(self.media_operation.as_str()) != expected.media_operation.as_deref()
            || self.artifact_field != expected.artifact_field
            || (!expected.auto_discover_action_type
                && Some(self.resolved_action_type.as_str())
                    != expected.action_body.get("type").and_then(Value::as_str))
        {
            return Err(failure());
        }
        Ok(())
    }
    pub(super) fn into_value(self) -> Value {
        // Only strings/Option<String> are serialized: failure would be an internal defect.
        serde_json::to_value(self).expect("Luma task consists only of serializable strings")
    }
}
