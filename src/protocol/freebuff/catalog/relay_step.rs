//! Caller-native relay intent is distinct from the default CLI harness and never executes tools.
use super::{
    root_context::{ContextState, RootRequest},
    BusinessError, BusinessRequests,
};
use serde_json::Value;
mod validation;

pub(crate) struct RelayStep {
    pub(super) body: Value,
    intent: Vec<u8>,
    pub(super) context: RootRequest,
}
impl RelayStep {
    pub(crate) fn validate(body: &Value) -> Result<(), BusinessError> {
        validation::validate(body)
    }
    pub(crate) fn intent(&self) -> &[u8] {
        &self.intent
    }
    fn build(
        mut body: Value,
        model: &str,
        context_limit: u64,
        limit: usize,
    ) -> Result<Self, BusinessError> {
        Self::validate(&body)?;
        let object = body.as_object_mut().expect("validated object");
        object.insert("model".into(), Value::String(model.into()));
        object.entry("stream").or_insert(Value::Bool(false));
        object.insert(
            "provider".into(),
            serde_json::json!({"data_collection":"deny"}),
        );
        let intent = super::completion::freeze(&body, limit)?;
        let indices: Vec<_> = (0..body["messages"]
            .as_array()
            .expect("validated messages")
            .len())
            .filter(|index| body["messages"][*index]["role"] != "system")
            .collect();
        let context = RootRequest::new(&body, &indices, context_limit)?;
        if ContextState::default().preview(&context).forecast.tokens > context_limit {
            return Err(BusinessError::ContextBudgetExceeded);
        }
        Ok(Self {
            body,
            intent,
            context,
        })
    }
}
impl BusinessRequests<'_> {
    pub(crate) fn prepare_relay_step(
        &self,
        body: Value,
        limit: usize,
    ) -> Result<RelayStep, BusinessError> {
        let model = self.root_model_input();
        RelayStep::build(
            body,
            model.model,
            model
                .compaction
                .and_then(|policy| policy.max_context_length)
                .unwrap_or(400000),
            limit,
        )
    }
}
#[cfg(test)]
mod tests;
